use rustc_hash::FxHashMap;

use crate::arena::{Arena, Id, Item};
use crate::builder::error::{PipelineError, PipelineStagedError};
use crate::intern::InternTable;
use crate::language::Dialect;
use crate::node::function::{
    CompileStage, Function, FunctionInfo, SpecializedFunction, SpecializedFunctionInfo,
    StagedFunction,
};
use crate::node::stmt::Statement;
use crate::node::symbol::GlobalSymbol;
use crate::signature::Signature;
use crate::stage::{HasStageInfo, StageMeta};

/// A compilation pipeline that holds stages and a global symbol table.
///
/// The global symbol table (`global_symbols`) provides cross-stage interning
/// for identifiers like function names. Stage-local symbols (SSA names, blocks)
/// remain in each stage's [`StageInfo`].
pub struct Pipeline<S> {
    stages: Vec<S>,
    functions: Arena<Function, FunctionInfo>,
    global_symbols: InternTable<String, GlobalSymbol>,
    name_index: FxHashMap<GlobalSymbol, Function>,
}

impl<S> Default for Pipeline<S> {
    fn default() -> Self {
        Self::new()
    }
}

impl<S> Pipeline<S> {
    pub fn new() -> Self {
        Self {
            stages: Vec::new(),
            functions: Arena::default(),
            global_symbols: InternTable::default(),
            name_index: FxHashMap::default(),
        }
    }

    /// Add a stage to the pipeline, returning its [`CompileStage`].
    ///
    /// *Non-builder variant.* Prefer the builder form (see [`Pipeline::add_stage`]
    /// in the `#[bon::bon]` block) when you want to set a stage name.
    pub fn add_stage_raw(&mut self, stage: S) -> CompileStage {
        let id = CompileStage::new(Id(self.stages.len()));
        self.stages.push(stage);
        id
    }

    /// Get a reference to a stage by its [`CompileStage`].
    pub fn stage(&self, id: CompileStage) -> Option<&S> {
        self.stages.get(Id::from(id).raw())
    }

    /// Get a mutable reference to a stage by its [`CompileStage`].
    pub fn stage_mut(&mut self, id: CompileStage) -> Option<&mut S> {
        self.stages.get_mut(Id::from(id).raw())
    }

    /// Get a slice of all stages.
    pub fn stages(&self) -> &[S] {
        &self.stages
    }

    /// Get a mutable slice of all stages.
    pub fn stages_mut(&mut self) -> &mut [S] {
        &mut self.stages
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
    /// should intern it first via [`Pipeline::intern`].
    ///
    /// Returns `None` if no function with the given name has been allocated
    /// via [`Pipeline::function`].
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
    /// Prefer [`Pipeline::staged_function`] which creates and links in one step.
    ///
    /// Equivalent to
    /// `pipeline.function_info_mut(func)?.add_staged_function(stage, sf)`.
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError::UnknownFunction`] if `func` does not exist.
    pub fn link(
        &mut self,
        func: Function,
        stage: CompileStage,
        sf: StagedFunction,
    ) -> Result<(), PipelineError> {
        self.functions
            .get_mut(func)
            .ok_or(PipelineError::UnknownFunction(func))?
            .add_staged_function(stage, sf);
        Ok(())
    }
}

#[bon::bon]
impl<S> Pipeline<S> {
    /// Add a stage to the pipeline, returning its [`CompileStage`].
    ///
    /// If a `name` is provided, it is interned into the global symbol table
    /// and set on the stage via [`StageMeta::set_stage_name`]. This lets the
    /// printing infrastructure display a readable label (e.g., `stage @llvm_ir`)
    /// instead of a numeric index.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let id = pipeline.add_stage().stage(ctx).new();                   // unnamed
    /// let id = pipeline.add_stage().stage(ctx).name("llvm_ir").new();   // named
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
        self.stages.push(stage);
        id
    }

    /// Allocate a new abstract function.
    ///
    /// Returns the [`Function`] identifier. If a name is provided it is
    /// interned into the global symbol table and stored on the [`FunctionInfo`].
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError::DuplicateFunctionName`] if a function with
    /// the same name already exists.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let func = pipeline.function().new()?;               // anonymous
    /// let func = pipeline.function().name("foo").new()?;   // named
    /// ```
    #[builder(finish_fn = new)]
    pub fn function(
        &mut self,
        #[builder(into)] name: Option<String>,
    ) -> Result<Function, PipelineError> {
        let sym = name
            .map(|n| {
                if let Some(existing) = self.global_symbols.lookup(n.as_str()) {
                    if self.name_index.contains_key(&existing) {
                        return Err(PipelineError::DuplicateFunctionName(existing));
                    }
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
    /// Returns [`PipelineStagedError::Pipeline`] if `func` or `stage` refers
    /// to an unknown ID, or [`PipelineStagedError::StagedFunction`] if the
    /// stage-level staged-function creation conflicts.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let sf = pipeline.staged_function()
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
    ) -> Result<StagedFunction, PipelineStagedError<L>>
    where
        S: HasStageInfo<L>,
    {
        // Read name from FunctionInfo (GlobalSymbol is Copy, borrow ends immediately).
        let name = self
            .functions
            .get(func)
            .ok_or(PipelineError::UnknownFunction(func))?
            .name();

        // Borrow the stage mutably to access its StageInfo.
        let stage_info = self
            .stages
            .get_mut(Id::from(stage).raw())
            .and_then(|s| HasStageInfo::<L>::try_stage_info_mut(s))
            .ok_or(PipelineError::UnknownFunction(func))?;

        // Delegate to StageInfo::staged_function builder.
        let sf = stage_info
            .staged_function()
            .maybe_name(name)
            .maybe_signature(signature)
            .maybe_specializations(specializations)
            .maybe_backedges(backedges)
            .new()?;

        // Auto-link the staged function to the abstract Function.
        self.functions
            .get_mut(func)
            .ok_or(PipelineError::UnknownFunction(func))?
            .add_staged_function(stage, sf);

        Ok(sf)
    }

    /// Create a function with a single staged function and specialization in one call.
    ///
    /// This is a convenience shorthand for the common case of creating the full
    /// three-level function hierarchy (Function → StagedFunction → SpecializedFunction)
    /// with a single body.
    ///
    /// # Errors
    ///
    /// Returns [`PipelineStagedError`] if the stage does not contain a
    /// `StageInfo` for the given dialect, or if a function with the same name
    /// already exists.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let (func, sf, spec) = pipeline.define_function::<MyDialect>()
    ///     .stage(stage_id)
    ///     .body(body_stmt)
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
        #[builder(into)] body: Statement,
    ) -> Result<(Function, StagedFunction, SpecializedFunction), PipelineStagedError<L>>
    where
        S: HasStageInfo<L>,
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
            .and_then(|s| HasStageInfo::<L>::try_stage_info_mut(s))
            .ok_or(PipelineError::UnknownFunction(func))?;

        // Omit signature — specialize defaults to the staged function's signature.
        let spec = stage_info
            .specialize()
            .staged_func(sf)
            .body(body)
            .new()
            .expect("specialization conflict on newly created staged function");

        Ok((func, sf, spec))
    }
}

#[cfg(test)]
mod tests {
    use super::Pipeline;
    use crate::builder::error::PipelineError;

    #[test]
    fn duplicate_function_names_are_forbidden() {
        let mut pipeline: Pipeline<()> = Pipeline::new();
        let _ = pipeline.function().name("foo").new().unwrap();
        let result = pipeline.function().name("foo").new();
        assert!(
            matches!(result, Err(PipelineError::DuplicateFunctionName(_))),
            "expected DuplicateFunctionName, got: {result:?}"
        );
    }

    #[test]
    fn function_by_name_is_stable_for_unique_names() {
        let mut pipeline: Pipeline<()> = Pipeline::new();
        let foo = pipeline.function().name("foo").new().unwrap();
        let sym = pipeline
            .lookup_symbol("foo")
            .expect("foo symbol should exist");
        assert_eq!(pipeline.function_by_name(sym), Some(foo));
    }

    #[test]
    fn pipeline_intern_resolve_roundtrip() {
        let mut pipeline: Pipeline<()> = Pipeline::new();
        let sym = pipeline.intern("hello");
        assert_eq!(pipeline.resolve(sym), Some("hello"));
    }

    #[test]
    fn pipeline_intern_is_idempotent() {
        let mut pipeline: Pipeline<()> = Pipeline::new();
        let s1 = pipeline.intern("hello");
        let s2 = pipeline.intern("hello");
        assert_eq!(s1, s2);
    }

    #[test]
    fn pipeline_anonymous_function() {
        let mut pipeline: Pipeline<()> = Pipeline::new();
        let f1 = pipeline.function().new().unwrap();
        let f2 = pipeline.function().new().unwrap();
        // Anonymous functions don't conflict
        assert_ne!(f1, f2);
    }

    #[test]
    fn pipeline_add_stage_and_access() {
        let mut pipeline: Pipeline<String> = Pipeline::new();
        let id = pipeline.add_stage_raw("stage0".to_string());
        assert_eq!(pipeline.stage(id), Some(&"stage0".to_string()));
    }

    #[test]
    fn pipeline_lookup_symbol_returns_none_for_missing() {
        let pipeline: Pipeline<()> = Pipeline::new();
        assert_eq!(pipeline.lookup_symbol("nonexistent"), None);
    }

    #[test]
    fn pipeline_function_by_name_returns_none_for_missing() {
        use crate::node::symbol::GlobalSymbol;
        let pipeline: Pipeline<()> = Pipeline::new();
        // GlobalSymbol(0) has never been allocated
        assert_eq!(pipeline.function_by_name(GlobalSymbol::from(0)), None);
    }

    #[test]
    fn pipeline_stage_returns_none_for_invalid_id() {
        use crate::arena::Id;
        use crate::node::function::CompileStage;

        let mut pipeline: Pipeline<String> = Pipeline::new();
        pipeline.add_stage_raw("stage0".to_string());
        let invalid = CompileStage::new(Id(999));
        assert_eq!(pipeline.stage(invalid), None);
    }

    #[test]
    fn pipeline_stage_mut_returns_none_for_invalid_id() {
        use crate::arena::Id;
        use crate::node::function::CompileStage;

        let mut pipeline: Pipeline<String> = Pipeline::new();
        pipeline.add_stage_raw("stage0".to_string());
        let invalid = CompileStage::new(Id(999));
        assert_eq!(pipeline.stage_mut(invalid), None);
    }

    #[test]
    fn pipeline_stages_slice() {
        let mut pipeline: Pipeline<String> = Pipeline::new();
        pipeline.add_stage_raw("a".to_string());
        pipeline.add_stage_raw("b".to_string());
        pipeline.add_stage_raw("c".to_string());
        assert_eq!(pipeline.stages().len(), 3);
        assert_eq!(pipeline.stages()[0], "a");
        assert_eq!(pipeline.stages()[1], "b");
        assert_eq!(pipeline.stages()[2], "c");
    }

    #[test]
    fn pipeline_link_unknown_function_returns_error() {
        use crate::arena::Id;
        use crate::node::function::{Function, StagedFunction};

        let mut pipeline: Pipeline<()> = Pipeline::new();
        let stage = pipeline.add_stage_raw(());
        // Create a Function ID that doesn't exist in the pipeline
        let invalid_func = Function::from(Id(999));
        let sf = StagedFunction::from(Id(0));
        let result = pipeline.link(invalid_func, stage, sf);
        assert!(
            matches!(result, Err(PipelineError::UnknownFunction(_))),
            "expected UnknownFunction, got: {result:?}"
        );
    }

    #[test]
    fn pipeline_function_info_returns_none_for_invalid() {
        use crate::arena::Id;
        use crate::node::function::Function;

        let pipeline: Pipeline<()> = Pipeline::new();
        let invalid_func = Function::from(Id(999));
        assert!(pipeline.function_info(invalid_func).is_none());
    }

    #[test]
    fn pipeline_resolve_returns_none_for_invalid_symbol() {
        use crate::node::symbol::GlobalSymbol;

        let pipeline: Pipeline<()> = Pipeline::new();
        // GlobalSymbol(42) was never interned
        let invalid_sym = GlobalSymbol::from(42);
        assert_eq!(pipeline.resolve(invalid_sym), None);
    }

    #[test]
    fn pipeline_multiple_anonymous_functions_are_distinct() {
        let mut pipeline: Pipeline<()> = Pipeline::new();
        let funcs: Vec<_> = (0..10)
            .map(|_| pipeline.function().new().unwrap())
            .collect();
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
            assert!(pipeline.function_info(*f).is_some());
        }
        // None should have names
        for f in &funcs {
            assert_eq!(pipeline.function_info(*f).unwrap().name(), None);
        }
    }
}
