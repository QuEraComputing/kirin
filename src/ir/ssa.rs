use crate::ir::{Block, Statement};
use crate::language::Language;

/// Represents a general SSA value that can be either
/// a value produced by a statement or an argument to a block.
///
/// If you are certain about the kind of SSA value, consider using
/// `ResultValue` or `BlockArgument` instead.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SSAValue(usize);

/// Represents a value produced by a statement.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ResultValue(usize);

/// Represents an argument to a block.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BlockArgument(usize);

/// Information about an SSA value in the database.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SSAInfo<L: Language> {
    id: SSAValue,
    name: Option<String>,
    ty: L::Type,
    kind: SSAKind,
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum SSAKind {
    Value(Statement),
    BlockArgument(Block),
}

impl From<ResultValue> for SSAValue {
    fn from(rv: ResultValue) -> Self {
        SSAValue(rv.0)
    }
}
impl From<BlockArgument> for SSAValue {
    fn from(ba: BlockArgument) -> Self {
        SSAValue(ba.0)
    }
}
