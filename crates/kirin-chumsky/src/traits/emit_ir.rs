use kirin_ir::{Dialect, StageInfo};
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

/// Context for emitting IR from parsed AST, tracking name mappings.
pub struct EmitContext<'a, L: Dialect> {
    pub stage: &'a mut StageInfo<L>,
    ssa_names: FxHashMap<String, kirin_ir::SSAValue>,
    block_names: FxHashMap<String, kirin_ir::Block>,
}

impl<'a, L: Dialect> EmitContext<'a, L> {
    pub fn new(stage: &'a mut StageInfo<L>) -> Self {
        Self {
            stage,
            ssa_names: FxHashMap::default(),
            block_names: FxHashMap::default(),
        }
    }

    pub fn lookup_ssa(&self, name: &str) -> Option<kirin_ir::SSAValue> {
        self.ssa_names.get(name).copied()
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
    L: Dialect<Type = T>,
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
