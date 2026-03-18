use kirin_ir::{BuilderStageInfo, Dialect, Placeholder, SSAKind, StageInfo};
use rustc_hash::FxHashMap;

/// Error type for IR emission from parsed AST nodes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EmitError {
    /// An SSA value was referenced but never defined.
    UndefinedSSA(String),
    /// A block label was referenced but never defined.
    UndefinedBlock(String),
    /// A custom error from dialect-specific emit logic.
    Custom(String),
}

impl std::fmt::Display for EmitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EmitError::UndefinedSSA(name) => write!(f, "undefined SSA value: %{name}"),
            EmitError::UndefinedBlock(name) => write!(f, "undefined block: ^{name}"),
            EmitError::Custom(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for EmitError {}

/// Type-erased function that creates a forward-reference SSA.
/// Stored in `EmitContext` so that `SSAValue::emit` doesn't need a `Placeholder` bound.
type ForwardRefCreator<L> = fn(&mut StageInfo<L>, &str) -> kirin_ir::SSAValue;

/// Context for emitting IR from parsed AST, tracking name mappings.
///
/// The `stage` field is a `&mut StageInfo<L>`, typically obtained by
/// dereferencing a [`BuilderStageInfo<L>`]. Callers constructing IR from text
/// should create a `BuilderStageInfo`, then pass a mutable reference to the
/// inner `StageInfo` (via `DerefMut`) to `EmitContext::new`.
pub struct EmitContext<'a, L: Dialect> {
    pub stage: &'a mut StageInfo<L>,
    ssa_names: FxHashMap<String, kirin_ir::SSAValue>,
    block_names: FxHashMap<String, kirin_ir::Block>,
    /// When set, undefined SSA references use this function to create
    /// forward-reference placeholders. Used for graph bodies with relaxed dominance.
    forward_ref_creator: Option<ForwardRefCreator<L>>,
}

impl<'a, L: Dialect> EmitContext<'a, L> {
    pub fn new(stage: &'a mut StageInfo<L>) -> Self {
        Self {
            stage,
            ssa_names: FxHashMap::default(),
            block_names: FxHashMap::default(),
            forward_ref_creator: None,
        }
    }

    /// Create an `EmitContext` from a [`BuilderStageInfo`].
    ///
    /// This is a convenience constructor equivalent to
    /// `EmitContext::new(&mut *builder_stage)`.
    pub fn from_builder(builder_stage: &'a mut BuilderStageInfo<L>) -> Self {
        Self::new(&mut *builder_stage)
    }

    pub fn lookup_ssa(&self, name: &str) -> Option<kirin_ir::SSAValue> {
        self.ssa_names.get(name).copied()
    }

    /// Look up an SSA value by name. If not found and a forward-reference
    /// creator is installed (relaxed dominance mode), creates a placeholder SSA.
    ///
    /// No `Placeholder` bound needed — the bound is captured in the creator
    /// function installed by [`set_relaxed_dominance`].
    pub fn resolve_ssa(&mut self, name: &str) -> Result<kirin_ir::SSAValue, EmitError> {
        if let Some(ssa) = self.ssa_names.get(name).copied() {
            return Ok(ssa);
        }
        if let Some(creator) = self.forward_ref_creator {
            let ssa = creator(self.stage, name);
            self.ssa_names.insert(name.to_string(), ssa);
            return Ok(ssa);
        }
        Err(EmitError::UndefinedSSA(name.to_string()))
    }

    /// Enable relaxed dominance mode: undefined SSA references create
    /// forward-reference `Unresolved(Result(0))` placeholders.
    pub fn set_relaxed_dominance(&mut self, relaxed: bool)
    where
        L::Type: Placeholder,
    {
        self.forward_ref_creator = if relaxed {
            Some(create_forward_ref::<L>)
        } else {
            None
        };
    }

    pub fn register_ssa(&mut self, name: String, ssa: kirin_ir::SSAValue) {
        self.ssa_names.insert(name, ssa);
    }

    pub fn lookup_block(&self, name: &str) -> Option<kirin_ir::Block> {
        self.block_names.get(name).copied()
    }

    pub fn register_block(&mut self, name: String, block: kirin_ir::Block) {
        self.block_names.insert(name, block);
    }
}

/// Create a forward-reference SSA for an undefined name in relaxed dominance mode.
fn create_forward_ref<L: Dialect>(stage: &mut StageInfo<L>, name: &str) -> kirin_ir::SSAValue
where
    L::Type: Placeholder,
{
    stage
        .ssa()
        .name(name.to_string())
        .ty(L::Type::placeholder())
        .kind(SSAKind::Unresolved(kirin_ir::ResolutionInfo::Result(0)))
        .new()
}

/// Trait for emitting IR nodes from parsed AST nodes.
pub trait EmitIR<L: Dialect> {
    type Output;
    fn emit(&self, ctx: &mut EmitContext<'_, L>) -> Result<Self::Output, EmitError>;
}

/// Marker trait for types that can be directly parsed into themselves.
///
/// This is used to provide identity conversion for types that parse directly
/// into themselves (like type lattice types and compile-time values) without
/// running into coherence issues with blanket implementations.
pub trait DirectlyParsable: Clone {}

/// Blanket implementation of EmitIR for types that implement DirectlyParsable.
///
/// This allows types to emit to themselves (identity conversion),
/// which is useful for type lattices and compile-time value types.
impl<T, L> EmitIR<L> for T
where
    L: Dialect,
    T: DirectlyParsable,
{
    type Output = T;

    fn emit(&self, _ctx: &mut EmitContext<'_, L>) -> Result<Self::Output, EmitError> {
        Ok(self.clone())
    }
}

impl<T, L> EmitIR<L> for Vec<T>
where
    L: Dialect,
    T: EmitIR<L>,
{
    type Output = Vec<T::Output>;

    fn emit(&self, ctx: &mut EmitContext<'_, L>) -> Result<Self::Output, EmitError> {
        self.iter().map(|item| item.emit(ctx)).collect()
    }
}

impl<T, L> EmitIR<L> for Option<T>
where
    L: Dialect,
    T: EmitIR<L>,
{
    type Output = Option<T::Output>;

    fn emit(&self, ctx: &mut EmitContext<'_, L>) -> Result<Self::Output, EmitError> {
        self.as_ref().map(|item| item.emit(ctx)).transpose()
    }
}
