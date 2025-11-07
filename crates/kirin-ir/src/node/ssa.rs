use std::collections::HashSet;

use crate::language::Language;

use super::{block::Block, stmt::Statement};

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

/// Represents a deleted SSA value. Used as a placeholder.
/// This points to the original SSA value's ID.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct DeletedSSAValue(usize);

/// Represents a test SSA value. Used in tests only.
/// This SSAValue may not exist in the SSA database.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct TestSSAValue(pub usize);

/// Information about an SSA value in the database.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SSAInfo<L: Language> {
    id: SSAValue,
    name: Option<String>,
    ty: L::Type,
    kind: SSAKind,
    uses: HashSet<Use>,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct Use {
    stmt: Statement,
    operand_index: usize,
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum SSAKind {
    Result(Statement),
    BlockArgument(Block),
    Deleted,
    Test,
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

impl From<TestSSAValue> for SSAValue {
    fn from(tsv: TestSSAValue) -> Self {
        SSAValue(tsv.0)
    }
}

impl From<TestSSAValue> for ResultValue {
    fn from(tsv: TestSSAValue) -> Self {
        ResultValue(tsv.0)
    }
}

impl From<TestSSAValue> for BlockArgument {
    fn from(tsv: TestSSAValue) -> Self {
        BlockArgument(tsv.0)
    }
}

impl From<TestSSAValue> for DeletedSSAValue {
    fn from(tsv: TestSSAValue) -> Self {
        DeletedSSAValue(tsv.0)
    }
}

impl From<TestSSAValue> for usize {
    fn from(tsv: TestSSAValue) -> Self {
        tsv.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ssa_value_conversion() {
        let rv = ResultValue(42);
        let ba = BlockArgument(84);

        let ssa_from_rv: SSAValue = rv.into();
        let ssa_from_ba: SSAValue = ba.into();

        assert_eq!(ssa_from_rv, SSAValue(42));
        assert_eq!(ssa_from_ba, SSAValue(84));
    }
}
