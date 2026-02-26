use crate::arena::{Arena, Id, Item};
use crate::builder::error::StagedFunctionError;
use crate::context::StageInfo;
use crate::intern::InternTable;
use crate::language::Dialect;
use crate::node::function::{
    CompileStage, Function, FunctionInfo, SpecializedFunctionInfo, StagedFunction,
};
use crate::node::symbol::GlobalSymbol;
use crate::signature::Signature;

/// Trait for types that contain a [`StageInfo`] for a specific dialect.
///
/// Parameterized by dialect type `L` so that enums with multiple stage variants
/// can implement it multiple times -- once per dialect:
///
/// ```ignore
/// enum Stage {
///     A(StageInfo<LangA>),
///     B(StageInfo<LangB>),
/// }
///
/// impl HasStageInfo<LangA> for Stage {
///     fn try_stage_info(&self) -> Option<&StageInfo<LangA>> {
///         match self { Stage::A(ctx) => Some(ctx), _ => None }
///     }
///     fn try_stage_info_mut(&mut self) -> Option<&mut StageInfo<LangA>> {
///         match self { Stage::A(ctx) => Some(ctx), _ => None }
///     }
/// }
/// ```
///
/// Composable via bounds: `S: HasStageInfo<LangA> + HasStageInfo<LangB>`.
pub trait HasStageInfo<L: Dialect> {
    /// Try to get a reference to the stage info for dialect `L`.
    ///
    /// Returns `None` if this stage does not contain stage info for dialect `L`
    /// (e.g., an enum variant for a different dialect).
    fn try_stage_info(&self) -> Option<&StageInfo<L>>;

    /// Try to get a mutable reference to the stage info for dialect `L`.
    ///
    /// Returns `None` if this stage does not contain stage info for dialect `L`.
    fn try_stage_info_mut(&mut self) -> Option<&mut StageInfo<L>>;
}

// Base case: StageInfo<L> trivially provides stage info for L.
impl<L: Dialect> HasStageInfo<L> for StageInfo<L> {
    fn try_stage_info(&self) -> Option<&StageInfo<L>> {
        Some(self)
    }

    fn try_stage_info_mut(&mut self) -> Option<&mut StageInfo<L>> {
        Some(self)
    }
}

/// Unified trait for stage identity and stage-container metadata.
///
/// Automatically implemented for [`StageInfo<L>`]. Allows [`Pipeline::add_stage`]
/// to set both a readable name and the numeric stage ID on the stage.
///
/// For enum stages, derive this trait with `#[derive(StageMeta)]`:
///
/// ```ignore
/// #[derive(StageMeta)]
/// enum Stage {
///     #[stage(name = "A")]
///     Parse(StageInfo<LangA>),
///     #[stage(name = "B")]
///     Lower(StageInfo<LangB>),
/// }
/// ```
pub trait StageMeta: Sized {
    /// The dialect dispatch list for `pipeline.parse(text)`.
    ///
    /// For heterogeneous pipelines use nested tuples: `(LangA, (LangB, ()))`.
    type Languages;

    /// Get the stage name, if set.
    fn stage_name(&self) -> Option<GlobalSymbol>;
    /// Set the stage name.
    fn set_stage_name(&mut self, name: Option<GlobalSymbol>);
    /// Get the compile-stage ID, if set.
    fn stage_id(&self) -> Option<CompileStage>;
    /// Set the compile-stage ID.
    fn set_stage_id(&mut self, id: Option<CompileStage>);

    /// Build a concrete stage from a parsed stage name (`@...`).
    fn from_stage_name(stage_name: &str) -> Result<Self, String>;

    /// The set of stage names this container recognizes (for typo suggestions).
    fn declared_stage_names() -> &'static [&'static str] {
        &[]
    }
}

impl<L: Dialect> StageMeta for StageInfo<L> {
    type Languages = (L, ());

    fn stage_name(&self) -> Option<GlobalSymbol> {
        self.name
    }

    fn set_stage_name(&mut self, name: Option<GlobalSymbol>) {
        self.name = name;
    }

    fn stage_id(&self) -> Option<CompileStage> {
        self.stage_id
    }

    fn set_stage_id(&mut self, id: Option<CompileStage>) {
        self.stage_id = id;
    }

    fn from_stage_name(_stage_name: &str) -> Result<Self, String> {
        Ok(StageInfo::default())
    }
}

/// A compilation pipeline that holds stages and a global symbol table.
///
/// The global symbol table (`global_symbols`) provides cross-stage interning
/// for identifiers like function names. Stage-local symbols (SSA names, blocks)
/// remain in each stage's [`StageInfo`].
pub struct Pipeline<S> {
    stages: Vec<S>,
    functions: Arena<Function, FunctionInfo>,
    global_symbols: InternTable<String, GlobalSymbol>,
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

    /// Intern a string into the global symbol table, returning a [`GlobalSymbol`].
    ///
    /// If the string has already been interned, the existing [`GlobalSymbol`] is returned.
    pub fn intern(&mut self, name: impl Into<String>) -> GlobalSymbol {
        self.global_symbols.intern(name.into())
    }

    /// Resolve a [`GlobalSymbol`] back to its string representation.
    pub fn resolve(&self, sym: GlobalSymbol) -> Option<&str> {
        self.global_symbols.resolve(sym).map(|s| s.as_str())
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
    /// This is a convenience shorthand for
    /// `pipeline.function_info_mut(func).unwrap().add_staged_function(stage, sf)`.
    ///
    /// # Panics
    ///
    /// Panics if `func` refers to an unknown [`Function`].
    pub fn link(&mut self, func: Function, stage: CompileStage, sf: StagedFunction) {
        self.functions
            .get_mut(func)
            .expect("unknown Function")
            .add_staged_function(stage, sf);
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
    /// # Examples
    ///
    /// ```ignore
    /// let func = pipeline.function().new();               // anonymous
    /// let func = pipeline.function().name("foo").new();   // named
    /// ```
    #[builder(finish_fn = new)]
    pub fn function(&mut self, #[builder(into)] name: Option<String>) -> Function {
        let sym = name.map(|n| self.global_symbols.intern(n));
        self.functions
            .alloc_with_id(|id| FunctionInfo::new(id, sym))
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
    /// # Panics
    ///
    /// Panics if `func` or `stage` refers to an unknown ID.
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
    ) -> Result<StagedFunction, StagedFunctionError<L>>
    where
        S: HasStageInfo<L>,
    {
        // Read name from FunctionInfo (GlobalSymbol is Copy, borrow ends immediately).
        let name = self.functions.get(func).expect("unknown Function").name();

        // Borrow the stage mutably to access its StageInfo.
        let stage_info = self
            .stages
            .get_mut(Id::from(stage).raw())
            .and_then(|s| HasStageInfo::<L>::try_stage_info_mut(s))
            .expect("invalid stage or stage does not contain a StageInfo for this dialect");

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
            .expect("unknown Function")
            .add_staged_function(stage, sf);

        Ok(sf)
    }
}
