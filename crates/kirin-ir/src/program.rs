use rustc_hash::FxHashMap;

use crate::arena::{Arena, Id, Item};
use crate::builder::error::{ProgramError, ProgramStagedError};
use crate::intern::InternTable;
use crate::language::Dialect;
use crate::node::function::{
    CompileStage, Function, FunctionInfo, SpecializedFunction, SpecializedFunctionInfo,
    StagedFunction,
};
use crate::node::stmt::Statement;
use crate::node::symbol::{GlobalSymbol, Symbol};
use crate::rewrite::Quarantined;
use crate::signature::Signature;
use crate::stage::{HasStageInfo, StageDispatchMiss, StageInfo, StageMeta, StageSlot, StageStatus};

/// The root IR database: every stage of a program, its cross-stage functions,
/// and a global symbol table.
///
/// It is not a pass pipeline: a stage is a whole IR level, and stages are kept
/// (not discarded after lowering) so a function can be respecialized at any of
/// them.
///
/// The global symbol table (`global_symbols`) provides cross-stage interning
/// for identifiers like function names. Stage-local symbols (SSA names, blocks)
/// remain in each stage's [`StageInfo`].
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Program<S> {
    stages: Vec<StageSlot<S>>,
    functions: Arena<Function, FunctionInfo>,
    global_symbols: InternTable<String, GlobalSymbol>,
    #[cfg_attr(feature = "serde", serde(skip))]
    name_index: FxHashMap<GlobalSymbol, Function>,
}

impl<S> Default for Program<S> {
    fn default() -> Self {
        Self::new()
    }
}

impl<S> Program<S> {
    /// Look up a stage by its human-readable name, returning its [`CompileStage`].
    ///
    /// Resolves the name against the slot rather than the stage, so a position
    /// stays addressable even once its stage has been lent to a pass or
    /// poisoned by a failed one. While the stage is present, the slot answers
    /// to the stage's own name, so this agrees with what the printer shows.
    ///
    /// Returns `None` if no stage has the given name.
    pub fn stage_by_name(&self, name: &str) -> Option<CompileStage>
    where
        S: StageMeta,
    {
        // Names are interned, so symbol equality is name equality — no need to
        // resolve every slot back to a string.
        let symbol = self.global_symbols.lookup(name)?;
        self.stages
            .iter()
            .position(|slot| slot.name() == Some(symbol))
            .map(|index| CompileStage::new(Id(index)))
    }

    /// Every position in the program with the name it answers to, in order.
    ///
    /// The reverse of [`Program::stage_by_name`], and like it this reads the
    /// name off the position rather than the stage, so a position a pass has
    /// taken or poisoned is still listed.
    ///
    /// Unlike [`Program::stages`], this yields *every* position, so the
    /// [`CompileStage`] it hands back is always the real one.
    pub fn stage_names(&self) -> impl Iterator<Item = (CompileStage, Option<GlobalSymbol>)> + '_
    where
        S: StageMeta,
    {
        self.stages
            .iter()
            .enumerate()
            .map(|(index, slot)| (CompileStage::new(Id(index)), slot.name()))
    }

    pub fn new() -> Self {
        Self {
            stages: Vec::new(),
            functions: Arena::default(),
            global_symbols: InternTable::default(),
            name_index: FxHashMap::default(),
        }
    }

    /// Add a stage to the program, returning its [`CompileStage`].
    ///
    /// *Non-builder variant.* Prefer the builder form (see [`Program::add_stage`]
    /// in the `#[bon::bon]` block) when you want to set a stage name.
    pub fn add_stage_raw(&mut self, stage: S) -> CompileStage {
        let id = CompileStage::new(Id(self.stages.len()));
        self.stages.push(StageSlot::present(stage));
        id
    }

    /// Get a reference to a stage by its [`CompileStage`].
    ///
    /// Returns `None` both when `id` names no position at all and when the
    /// position exists but the stage is either transferred to or poisoned
    /// by a pass. Use [`Program::stage_status`] to tell those apart.
    pub fn stage(&self, id: CompileStage) -> Option<&S> {
        self.stage_or_miss(id).ok()
    }

    /// Get a mutable reference to a stage by its [`CompileStage`].
    ///
    /// Shares [`Program::stage`]'s `None` cases.
    pub fn stage_mut(&mut self, id: CompileStage) -> Option<&mut S> {
        self.stage_or_miss_mut(id).ok()
    }

    /// The slot at `id`, whatever state it is in.
    ///
    /// Crate-private: a slot exposes its own state and its
    /// [`Quarantined`](crate::Quarantined), which
    /// [`Program::stage`] deliberately does not. Callers outside the crate go
    /// through the accessors that answer one question each.
    pub(crate) fn slot(&self, id: CompileStage) -> Option<&StageSlot<S>> {
        self.stages.get(Id::from(id).raw())
    }

    /// The slot at `id`, mutably, whatever state it is in.
    ///
    /// The pass wrapper needs this rather than [`Program::stage_mut`]: running
    /// a pass moves the stage out of its slot and back, which is a change of
    /// slot *state*, not a mutation of the stage.
    pub(crate) fn slot_mut(&mut self, id: CompileStage) -> Option<&mut StageSlot<S>> {
        self.stages.get_mut(Id::from(id).raw())
    }

    /// Which state the position at `id` is in, or `None` if there is no such
    /// position.
    ///
    /// [`Program::stage`] answers "can I use this stage?"; this answers "why
    /// not?"
    pub fn stage_status(&self, id: CompileStage) -> Option<StageStatus> {
        self.slot(id).map(StageSlot::status)
    }

    /// The artifact left by a pass that failed at `id`, if one did.
    ///
    /// `None` when `id` names no position, or when its stage is still usable.
    /// This is the only way to read the cause, mutation events, and report
    /// behind [`StagePassError::PassFailed`](crate::StagePassError): the stage
    /// itself stays inside the [`Quarantined`](crate::Quarantined) and is never
    /// lent out again.
    pub fn quarantined(&self, id: CompileStage) -> Option<&Quarantined<S>> {
        self.slot(id).and_then(StageSlot::quarantined)
    }

    /// The stage at `id`, or the reason dispatch cannot reach it.
    ///
    /// The same lookup as [`Program::stage`], but it keeps *why* it failed
    /// rather than folding both reasons into `None`. Dispatch reports that
    /// reason to its caller, so a poisoned position is not mistaken for a
    /// missing one.
    pub(crate) fn stage_or_miss(&self, id: CompileStage) -> Result<&S, StageDispatchMiss> {
        let slot = self.slot(id).ok_or(StageDispatchMiss::MissingStage)?;
        slot.stage().ok_or(StageDispatchMiss::StageUnavailable)
    }

    /// Mutable counterpart of [`Program::stage_or_miss`].
    pub(crate) fn stage_or_miss_mut(
        &mut self,
        id: CompileStage,
    ) -> Result<&mut S, StageDispatchMiss> {
        let slot = self.slot_mut(id).ok_or(StageDispatchMiss::MissingStage)?;
        slot.stage_mut().ok_or(StageDispatchMiss::StageUnavailable)
    }

    /// Iterate every stage the program still holds, in program order.
    ///
    /// Skips positions whose stage is absent, so the yielded items are always
    /// usable. Positions are not renumbered: use [`Program::stage_by_name`] or
    /// a [`CompileStage`] to address one.
    pub fn stages(&self) -> impl Iterator<Item = &S> {
        self.stages.iter().filter_map(StageSlot::stage)
    }

    /// Iterate every stage the program still holds, mutably, in program order.
    pub fn stages_mut(&mut self) -> impl Iterator<Item = &mut S> {
        self.stages.iter_mut().filter_map(StageSlot::stage_mut)
    }

    /// Get the [`FunctionInfo`] for a function by its [`Function`] ID.
    pub fn function_info(&self, func: Function) -> Option<&Item<FunctionInfo>> {
        self.functions.get(func)
    }

    /// Get mutable [`FunctionInfo`] for a function by its [`Function`] ID.
    pub fn function_info_mut(&mut self, func: Function) -> Option<&mut Item<FunctionInfo>> {
        self.functions.get_mut(func)
    }

    /// Get a reference to the function arena.
    pub fn function_arena(&self) -> &Arena<Function, FunctionInfo> {
        &self.functions
    }

    /// Look up a function by its interned name in O(1) time.
    ///
    /// The `name` parameter is a [`GlobalSymbol`] — callers that have a string
    /// should intern it first via [`Program::intern`].
    ///
    /// Returns `None` if no function with the given name has been allocated
    /// via [`Program::function`].
    pub fn function_by_name(&self, name: GlobalSymbol) -> Option<Function> {
        self.name_index.get(&name).copied()
    }

    /// Intern a string into the global symbol table, returning a [`GlobalSymbol`].
    ///
    /// If the string has already been interned, the existing [`GlobalSymbol`] is returned.
    pub fn intern(&mut self, name: impl Into<String>) -> GlobalSymbol {
        let name = name.into();
        if let Some(existing) = self.global_symbols.lookup(name.as_str()) {
            return existing;
        }
        self.global_symbols.intern(name)
    }

    /// Resolve a [`GlobalSymbol`] back to its string representation.
    pub fn resolve(&self, sym: GlobalSymbol) -> Option<&str> {
        self.global_symbols.resolve(sym).map(|s| s.as_str())
    }

    /// Look up a [`GlobalSymbol`] by its string without interning.
    ///
    /// Returns `None` if the name has never been interned.
    pub fn lookup_symbol(&self, name: &str) -> Option<GlobalSymbol> {
        self.global_symbols.lookup(name)
    }

    /// Resolve a function name to its [`Function`] id by chaining
    /// [`lookup_symbol`](Self::lookup_symbol) and
    /// [`function_by_name`](Self::function_by_name).
    pub fn lookup_function_by_name(&self, name: &str) -> Option<Function> {
        let sym = self.lookup_symbol(name)?;
        self.function_by_name(sym)
    }

    /// Get a reference to the global symbol table.
    pub fn global_symbols(&self) -> &InternTable<String, GlobalSymbol> {
        &self.global_symbols
    }

    /// Get a mutable reference to the global symbol table.
    pub fn global_symbols_mut(&mut self) -> &mut InternTable<String, GlobalSymbol> {
        &mut self.global_symbols
    }

    /// Link a [`StagedFunction`] to an abstract [`Function`] at the given stage.
    ///
    /// This is a low-level method for manually linking a staged function that was
    /// created directly on a [`StageInfo`] (via [`StageInfo::staged_function`]).
    /// Prefer [`Program::staged_function`] which creates and links in one step.
    ///
    /// Equivalent to
    /// `program.function_info_mut(func)?.add_staged_function(stage, sf)`.
    ///
    /// # Errors
    ///
    /// Returns [`ProgramError::UnknownFunction`] if `func` does not exist.
    pub fn link(
        &mut self,
        func: Function,
        stage: CompileStage,
        sf: StagedFunction,
    ) -> Result<(), ProgramError> {
        self.functions
            .get_mut(func)
            .ok_or(ProgramError::UnknownFunction(func))?
            .add_staged_function(stage, sf);
        Ok(())
    }

    /// Resolve a named function at a given stage to its [`StagedFunction`].
    ///
    /// Combines [`lookup_symbol`](Self::lookup_symbol) →
    /// [`function_by_name`](Self::function_by_name) →
    /// [`function_info`](Self::function_info) → staged function lookup
    /// into a single call.
    ///
    /// Returns `None` if any step in the chain fails.
    pub fn resolve_staged_function(
        &self,
        func_name: &str,
        stage: CompileStage,
    ) -> Option<StagedFunction> {
        let sym = self.lookup_symbol(func_name)?;
        let func = self.function_by_name(sym)?;
        let info = self.function_info(func)?;
        info.staged_function(stage)
    }

    /// Resolve a stage-local [`Symbol`] to a program-level [`Function`].
    ///
    /// The lookup chain is:
    /// stage-local symbol -> interned name -> global symbol -> function id.
    ///
    /// Returns `None` if any link in that chain is missing.
    pub fn resolve_function<L: Dialect>(
        &self,
        stage: &StageInfo<L>,
        target: Symbol,
    ) -> Option<Function> {
        let target_name = stage.symbol_table().resolve(target)?;
        let global_symbol = self.lookup_symbol(target_name)?;
        self.function_by_name(global_symbol)
    }
}

#[bon::bon]
impl<S> Program<S> {
    /// Add a stage to the program, returning its [`CompileStage`].
    ///
    /// If a `name` is provided, it is interned into the global symbol table
    /// and set on the stage via [`StageMeta::set_stage_name`]. This lets the
    /// printing infrastructure display a readable label (e.g., `stage @llvm_ir`)
    /// instead of a numeric index.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let id = program.add_stage().stage(ctx).new();                   // unnamed
    /// let id = program.add_stage().stage(ctx).name("llvm_ir").new();   // named
    /// ```
    #[builder(finish_fn = new)]
    pub fn add_stage(&mut self, mut stage: S, #[builder(into)] name: Option<String>) -> CompileStage
    where
        S: StageMeta,
    {
        let id = CompileStage::new(Id(self.stages.len()));
        stage.set_stage_id(Some(id));
        if let Some(n) = name {
            let sym = self.global_symbols.intern(n);
            stage.set_stage_name(Some(sym));
        }
        self.stages.push(StageSlot::present(stage));
        id
    }

    /// Allocate a new abstract function.
    ///
    /// Returns the [`Function`] identifier. If a name is provided it is
    /// interned into the global symbol table and stored on the [`FunctionInfo`].
    ///
    /// # Errors
    ///
    /// Returns [`ProgramError::DuplicateFunctionName`] if a function with
    /// the same name already exists.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let func = program.function().new()?;               // anonymous
    /// let func = program.function().name("foo").new()?;   // named
    /// ```
    #[builder(finish_fn = new)]
    pub fn function(
        &mut self,
        #[builder(into)] name: Option<String>,
    ) -> Result<Function, ProgramError> {
        let sym = name
            .map(|n| {
                if let Some(existing) = self.global_symbols.lookup(n.as_str())
                    && self.name_index.contains_key(&existing)
                {
                    return Err(ProgramError::DuplicateFunctionName(existing));
                }
                Ok(self.global_symbols.intern(n))
            })
            .transpose()?;
        let func = self
            .functions
            .alloc_with_id(|id| FunctionInfo::new(id, sym));
        if let Some(s) = sym {
            self.name_index.insert(s, func);
        }
        Ok(func)
    }

    /// Create a staged function for an abstract [`Function`] at the given stage.
    ///
    /// The name is automatically derived from the [`FunctionInfo`] so callers
    /// never need to pass a [`GlobalSymbol`] manually. After creation the
    /// staged function is automatically linked to the [`FunctionInfo`].
    ///
    /// Delegates to [`crate::StageInfo::staged_function`] internally, so all the same
    /// duplicate-detection and policy rules apply.
    ///
    /// # Errors
    ///
    /// Returns [`ProgramStagedError::Program`] if `func` or `stage` refers
    /// to an unknown ID, or [`ProgramStagedError::StagedFunction`] if the
    /// stage-level staged-function creation conflicts.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let sf = program.staged_function()
    ///     .func(func)
    ///     .stage(stage_id)
    ///     .signature(sig)
    ///     .new()
    ///     .unwrap();
    /// ```
    #[builder(finish_fn = new)]
    pub fn staged_function<L: Dialect>(
        &mut self,
        func: Function,
        stage: CompileStage,
        signature: Option<Signature<L::Type>>,
        specializations: Option<Vec<SpecializedFunctionInfo<L>>>,
        backedges: Option<Vec<StagedFunction>>,
    ) -> Result<StagedFunction, ProgramStagedError<L>>
    where
        S: HasStageInfo<L>,
        L::Type: crate::Placeholder,
    {
        // Read name from FunctionInfo (GlobalSymbol is Copy, borrow ends immediately).
        let name = self
            .functions
            .get(func)
            .ok_or(ProgramError::UnknownFunction(func))?
            .name();

        // Borrow the stage mutably to access its StageInfo.
        let stage_info = self
            .stages
            .get_mut(Id::from(stage).raw())
            .and_then(StageSlot::stage_mut)
            .and_then(|s| HasStageInfo::<L>::try_stage_info_mut(s))
            .ok_or(ProgramError::UnknownFunction(func))?;

        // Delegate to BuilderStageInfo::staged_function via with_builder.
        let sf = stage_info.with_builder(|b| {
            b.staged_function()
                .maybe_name(name)
                .maybe_signature(signature)
                .maybe_specializations(specializations)
                .maybe_backedges(backedges)
                .new()
        })?;

        // Auto-link the staged function to the abstract Function.
        self.functions
            .get_mut(func)
            .ok_or(ProgramError::UnknownFunction(func))?
            .add_staged_function(stage, sf);

        Ok(sf)
    }

    /// Create a function with a single staged function and specialization in one call.
    ///
    /// This is a convenience shorthand for the common case of creating the full
    /// three-level function hierarchy (Function → StagedFunction → SpecializedFunction)
    /// with a single definition.
    ///
    /// # Errors
    ///
    /// Returns [`ProgramStagedError`] if the stage does not contain a
    /// `StageInfo` for the given dialect, or if a function with the same name
    /// already exists.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let (func, sf, spec) = program.define_function::<MyDialect>()
    ///     .stage(stage_id)
    ///     .definition(definition)
    ///     .name("my_func")
    ///     .signature(sig)
    ///     .new()
    ///     .unwrap();
    /// ```
    #[builder(finish_fn = new)]
    pub fn define_function<L: Dialect>(
        &mut self,
        #[builder(into)] name: Option<String>,
        stage: CompileStage,
        signature: Option<Signature<L::Type>>,
        #[builder(into)] definition: Statement,
    ) -> Result<(Function, StagedFunction, SpecializedFunction), ProgramStagedError<L>>
    where
        S: HasStageInfo<L>,
        L::Type: crate::Placeholder,
    {
        let func = self.function().maybe_name(name).new()?;

        let sf = self
            .staged_function::<L>()
            .func(func)
            .stage(stage)
            .maybe_signature(signature)
            .new()?;

        let stage_info = self
            .stages
            .get_mut(Id::from(stage).raw())
            .and_then(StageSlot::stage_mut)
            .and_then(|s| HasStageInfo::<L>::try_stage_info_mut(s))
            .ok_or(ProgramError::UnknownFunction(func))?;

        // Omit signature — specialize defaults to the staged function's signature.
        let spec = stage_info
            .with_builder(|b| b.specialize().staged_func(sf).definition(definition).new())
            .expect("specialization conflict on newly created staged function");

        Ok((func, sf, spec))
    }
}

#[cfg(test)]
mod tests {
    use super::Program;
    use crate::builder::error::ProgramError;

    #[test]
    fn duplicate_function_names_are_forbidden() {
        let mut program: Program<()> = Program::new();
        let _ = program.function().name("foo").new().unwrap();
        let result = program.function().name("foo").new();
        assert!(
            matches!(result, Err(ProgramError::DuplicateFunctionName(_))),
            "expected DuplicateFunctionName, got: {result:?}"
        );
    }

    #[test]
    fn function_by_name_is_stable_for_unique_names() {
        let mut program: Program<()> = Program::new();
        let foo = program.function().name("foo").new().unwrap();
        let sym = program
            .lookup_symbol("foo")
            .expect("foo symbol should exist");
        assert_eq!(program.function_by_name(sym), Some(foo));
    }

    #[test]
    fn program_intern_resolve_roundtrip() {
        let mut program: Program<()> = Program::new();
        let sym = program.intern("hello");
        assert_eq!(program.resolve(sym), Some("hello"));
    }

    #[test]
    fn program_intern_is_idempotent() {
        let mut program: Program<()> = Program::new();
        let s1 = program.intern("hello");
        let s2 = program.intern("hello");
        assert_eq!(s1, s2);
    }

    #[test]
    fn program_anonymous_function() {
        let mut program: Program<()> = Program::new();
        let f1 = program.function().new().unwrap();
        let f2 = program.function().new().unwrap();
        // Anonymous functions don't conflict
        assert_ne!(f1, f2);
    }

    #[test]
    fn program_add_stage_and_access() {
        let mut program: Program<String> = Program::new();
        let id = program.add_stage_raw("stage0".to_string());
        assert_eq!(program.stage(id), Some(&"stage0".to_string()));
    }

    #[test]
    fn program_lookup_symbol_returns_none_for_missing() {
        let program: Program<()> = Program::new();
        assert_eq!(program.lookup_symbol("nonexistent"), None);
    }

    #[test]
    fn program_function_by_name_returns_none_for_missing() {
        use crate::node::symbol::GlobalSymbol;
        let program: Program<()> = Program::new();
        // GlobalSymbol(0) has never been allocated
        assert_eq!(program.function_by_name(GlobalSymbol::from(0)), None);
    }

    #[test]
    fn program_stage_returns_none_for_invalid_id() {
        use crate::arena::Id;
        use crate::node::function::CompileStage;

        let mut program: Program<String> = Program::new();
        program.add_stage_raw("stage0".to_string());
        let invalid = CompileStage::new(Id(999));
        assert_eq!(program.stage(invalid), None);
    }

    #[test]
    fn program_stage_mut_returns_none_for_invalid_id() {
        use crate::arena::Id;
        use crate::node::function::CompileStage;

        let mut program: Program<String> = Program::new();
        program.add_stage_raw("stage0".to_string());
        let invalid = CompileStage::new(Id(999));
        assert_eq!(program.stage_mut(invalid), None);
    }

    #[test]
    fn program_stages_slice() {
        let mut program: Program<String> = Program::new();
        program.add_stage_raw("a".to_string());
        program.add_stage_raw("b".to_string());
        program.add_stage_raw("c".to_string());
        let stages: Vec<&String> = program.stages().collect();
        assert_eq!(stages, vec!["a", "b", "c"]);
    }

    #[test]
    fn program_link_unknown_function_returns_error() {
        use crate::arena::Id;
        use crate::node::function::{Function, StagedFunction};

        let mut program: Program<()> = Program::new();
        let stage = program.add_stage_raw(());
        // Create a Function ID that doesn't exist in the program
        let invalid_func = Function::from(Id(999));
        let sf = StagedFunction::from(Id(0));
        let result = program.link(invalid_func, stage, sf);
        assert!(
            matches!(result, Err(ProgramError::UnknownFunction(_))),
            "expected UnknownFunction, got: {result:?}"
        );
    }

    #[test]
    fn program_function_info_returns_none_for_invalid() {
        use crate::arena::Id;
        use crate::node::function::Function;

        let program: Program<()> = Program::new();
        let invalid_func = Function::from(Id(999));
        assert!(program.function_info(invalid_func).is_none());
    }

    #[test]
    fn program_resolve_returns_none_for_invalid_symbol() {
        use crate::node::symbol::GlobalSymbol;

        let program: Program<()> = Program::new();
        // GlobalSymbol(42) was never interned
        let invalid_sym = GlobalSymbol::from(42);
        assert_eq!(program.resolve(invalid_sym), None);
    }

    // --- Minimal StageMeta impl for stage_by_name tests ---
    use crate::node::function::CompileStage;
    use crate::node::symbol::GlobalSymbol;

    #[derive(Debug)]
    struct NamedStage {
        name: Option<GlobalSymbol>,
        id: Option<CompileStage>,
    }

    impl NamedStage {
        fn new() -> Self {
            Self {
                name: None,
                id: None,
            }
        }
    }

    impl crate::stage::StageMeta for NamedStage {
        type Languages = ();

        fn stage_name(&self) -> Option<GlobalSymbol> {
            self.name
        }

        fn set_stage_name(&mut self, name: Option<GlobalSymbol>) {
            self.name = name;
        }

        fn stage_id(&self) -> Option<CompileStage> {
            self.id
        }

        fn set_stage_id(&mut self, id: Option<CompileStage>) {
            self.id = id;
        }

        fn from_stage_name(_stage_name: &str) -> Result<Self, String> {
            Ok(NamedStage::new())
        }

        fn declared_stage_names() -> &'static [&'static str] {
            &[]
        }
    }

    #[test]
    fn stage_by_name_returns_matching_stage() {
        let mut program: Program<NamedStage> = Program::new();
        let id = program
            .add_stage()
            .stage(NamedStage::new())
            .name("source")
            .new();
        assert_eq!(program.stage_by_name("source"), Some(id));
    }

    #[test]
    fn stage_by_name_returns_none_for_missing() {
        let mut program: Program<NamedStage> = Program::new();
        program
            .add_stage()
            .stage(NamedStage::new())
            .name("source")
            .new();
        assert_eq!(program.stage_by_name("nonexistent"), None);
    }

    #[test]
    fn stage_by_name_returns_none_for_unnamed_stage() {
        let mut program: Program<NamedStage> = Program::new();
        // Add an unnamed stage (no name set via builder)
        program.add_stage_raw(NamedStage::new());
        assert_eq!(program.stage_by_name("anything"), None);
    }

    #[test]
    fn resolve_staged_function_full_chain() {
        use crate::arena::Id;
        use crate::node::function::StagedFunction;

        let mut program: Program<()> = Program::new();
        let stage = program.add_stage_raw(());
        let func = program.function().name("main").new().unwrap();
        let sf = StagedFunction::from(Id(42));
        program.link(func, stage, sf).unwrap();

        assert_eq!(program.resolve_staged_function("main", stage), Some(sf));
    }

    #[test]
    fn resolve_staged_function_missing_name() {
        let mut program: Program<()> = Program::new();
        let stage = program.add_stage_raw(());
        assert_eq!(program.resolve_staged_function("missing", stage), None);
    }

    #[test]
    fn resolve_staged_function_missing_stage() {
        use crate::arena::Id;
        use crate::node::function::CompileStage;

        let mut program: Program<()> = Program::new();
        let _ = program.function().name("main").new().unwrap();
        let unlinked_stage = CompileStage::new(Id(99));
        assert_eq!(
            program.resolve_staged_function("main", unlinked_stage),
            None
        );
    }

    #[test]
    fn resolve_staged_function_not_linked_at_stage() {
        let mut program: Program<()> = Program::new();
        let stage_a = program.add_stage_raw(());
        let stage_b = program.add_stage_raw(());
        let func = program.function().name("foo").new().unwrap();
        // Link to stage_a only
        use crate::arena::Id;
        use crate::node::function::StagedFunction;
        let sf = StagedFunction::from(Id(0));
        program.link(func, stage_a, sf).unwrap();

        // Should find at stage_a but not stage_b
        assert_eq!(program.resolve_staged_function("foo", stage_a), Some(sf));
        assert_eq!(program.resolve_staged_function("foo", stage_b), None);
    }

    #[test]
    fn program_multiple_anonymous_functions_are_distinct() {
        let mut program: Program<()> = Program::new();
        let funcs: Vec<_> = (0..10).map(|_| program.function().new().unwrap()).collect();
        // All should be distinct
        for i in 0..funcs.len() {
            for j in (i + 1)..funcs.len() {
                assert_ne!(
                    funcs[i], funcs[j],
                    "anonymous functions {i} and {j} collided"
                );
            }
        }
        // All should have function_info
        for f in &funcs {
            assert!(program.function_info(*f).is_some());
        }
        // None should have names
        for f in &funcs {
            assert_eq!(program.function_info(*f).unwrap().name(), None);
        }
    }
}

#[cfg(feature = "serde")]
impl<'de, S: serde::Deserialize<'de>> serde::Deserialize<'de> for Program<S> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(serde::Deserialize)]
        struct Data<S> {
            stages: Vec<S>,
            functions: Arena<Function, FunctionInfo>,
            global_symbols: InternTable<String, GlobalSymbol>,
        }
        let data = Data::deserialize(deserializer)?;
        let mut name_index = FxHashMap::default();
        for function in data.functions.iter() {
            if let Some(name) = function.name() {
                if data.global_symbols.resolve(name).is_none() {
                    return Err(serde::de::Error::custom("unknown global function symbol"));
                }
                if name_index.insert(name, function.id()).is_some() {
                    return Err(serde::de::Error::custom("duplicate function name"));
                }
            }
        }
        Ok(Self {
            // Only present stages serialize, so every restored position is one.
            stages: data.stages.into_iter().map(StageSlot::present).collect(),
            functions: data.functions,
            global_symbols: data.global_symbols,
            name_index,
        })
    }
}
