use crate::arena::{Arena, Id, Item};
use crate::builder::error::StagedFunctionError;
use crate::context::Context;
use crate::intern::InternTable;
use crate::language::Dialect;
use crate::node::function::{
    CompileStageId, Function, FunctionInfo, SpecializedFunctionInfo, StagedFunction,
};
use crate::node::symbol::GlobalSymbol;
use crate::signature::Signature;

/// Trait for types that contain a [`Context`] for a specific dialect.
///
/// Parameterized by dialect type `L` so that enums with multiple stage variants
/// can implement it multiple times -- once per dialect:
///
/// ```ignore
/// enum Stage {
///     A(Context<LangA>),
///     B(Context<LangB>),
/// }
///
/// impl CompileStage<LangA> for Stage {
///     fn try_context(&self) -> Option<&Context<LangA>> {
///         match self { Stage::A(ctx) => Some(ctx), _ => None }
///     }
///     fn try_context_mut(&mut self) -> Option<&mut Context<LangA>> {
///         match self { Stage::A(ctx) => Some(ctx), _ => None }
///     }
/// }
/// ```
///
/// Composable via bounds: `S: CompileStage<LangA> + CompileStage<LangB>`.
pub trait CompileStage<L: Dialect> {
    /// Try to get a reference to the context for dialect `L`.
    ///
    /// Returns `None` if this stage does not contain a context for dialect `L`
    /// (e.g., an enum variant for a different dialect).
    fn try_context(&self) -> Option<&Context<L>>;

    /// Try to get a mutable reference to the context for dialect `L`.
    ///
    /// Returns `None` if this stage does not contain a context for dialect `L`.
    fn try_context_mut(&mut self) -> Option<&mut Context<L>>;
}

// Base case: Context<L> trivially is a compile stage for L.
impl<L: Dialect> CompileStage<L> for Context<L> {
    fn try_context(&self) -> Option<&Context<L>> {
        Some(self)
    }

    fn try_context_mut(&mut self) -> Option<&mut Context<L>> {
        Some(self)
    }
}

/// Non-generic trait for stage identity (name and compile-stage ID).
///
/// Automatically implemented for [`Context<L>`]. Allows [`Pipeline::add_stage`]
/// to set both a readable name and the numeric stage ID on the stage.
///
/// For enum stages, implement this trait by delegating to the
/// active variant's context.
pub trait StageIdentity {
    /// Get the stage name, if set.
    fn stage_name(&self) -> Option<GlobalSymbol>;
    /// Set the stage name.
    fn set_stage_name(&mut self, name: Option<GlobalSymbol>);
    /// Get the compile-stage ID, if set.
    fn stage_id(&self) -> Option<CompileStageId>;
    /// Set the compile-stage ID.
    fn set_stage_id(&mut self, id: Option<CompileStageId>);
}

impl<L: Dialect> StageIdentity for Context<L> {
    fn stage_name(&self) -> Option<GlobalSymbol> {
        self.name
    }

    fn set_stage_name(&mut self, name: Option<GlobalSymbol>) {
        self.name = name;
    }

    fn stage_id(&self) -> Option<CompileStageId> {
        self.stage_id
    }

    fn set_stage_id(&mut self, id: Option<CompileStageId>) {
        self.stage_id = id;
    }
}

/// A compilation pipeline that holds stages and a global symbol table.
///
/// The global symbol table (`global_symbols`) provides cross-stage interning
/// for identifiers like function names. Stage-local symbols (SSA names, blocks)
/// remain in each stage's [`Context`].
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

    /// Add a stage to the pipeline, returning its [`CompileStageId`].
    ///
    /// *Non-builder variant.* Prefer the builder form (see [`Pipeline::add_stage`]
    /// in the `#[bon::bon]` block) when you want to set a stage name.
    pub fn add_stage_raw(&mut self, stage: S) -> CompileStageId {
        let id = CompileStageId::new(Id(self.stages.len()));
        self.stages.push(stage);
        id
    }

    /// Get a reference to a stage by its [`CompileStageId`].
    pub fn stage(&self, id: CompileStageId) -> Option<&S> {
        self.stages.get(Id::from(id).raw())
    }

    /// Get a mutable reference to a stage by its [`CompileStageId`].
    pub fn stage_mut(&mut self, id: CompileStageId) -> Option<&mut S> {
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
    pub fn link(&mut self, func: Function, stage: CompileStageId, sf: StagedFunction) {
        self.functions
            .get_mut(func)
            .expect("unknown Function")
            .add_staged_function(stage, sf);
    }
}

#[bon::bon]
impl<S> Pipeline<S> {
    /// Add a stage to the pipeline, returning its [`CompileStageId`].
    ///
    /// If a `name` is provided, it is interned into the global symbol table
    /// and set on the stage via [`StageIdentity::set_stage_name`]. This lets the
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
    pub fn add_stage(
        &mut self,
        mut stage: S,
        #[builder(into)] name: Option<String>,
    ) -> CompileStageId
    where
        S: StageIdentity,
    {
        let id = CompileStageId::new(Id(self.stages.len()));
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
    /// Delegates to [`Context::staged_function`] internally, so all the same
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
        stage: CompileStageId,
        signature: Option<Signature<L::Type>>,
        specializations: Option<Vec<SpecializedFunctionInfo<L>>>,
        backedges: Option<Vec<StagedFunction>>,
    ) -> Result<StagedFunction, StagedFunctionError<L>>
    where
        S: CompileStage<L>,
    {
        // Read name from FunctionInfo (GlobalSymbol is Copy, borrow ends immediately).
        let name = self.functions.get(func).expect("unknown Function").name();

        // Borrow the stage mutably to access its Context.
        let ctx = self
            .stages
            .get_mut(Id::from(stage).raw())
            .and_then(|s| CompileStage::<L>::try_context_mut(s))
            .expect("invalid stage or stage does not contain a Context for this dialect");

        // Delegate to Context::staged_function builder.
        let sf = ctx
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
