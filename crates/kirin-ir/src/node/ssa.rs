use std::collections::HashSet;

use crate::{Symbol, language::Language};

use super::{block::Block, stmt::StatementId};

/// Represents a general SSA value that can be either
/// a value produced by a statement or an argument to a block.
///
/// If you are certain about the kind of SSA value, consider using
/// `ResultValue` or `BlockArgument` instead.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SSAValue(pub(crate) usize);

impl SSAValue {
    /// Get the underlying ID of the SSA value.
    pub fn id(&self) -> usize {
        self.0
    }
}

/// Represents a value produced by a statement.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ResultValue(pub(crate) usize);

/// Represents an argument to a block.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BlockArgument(pub(crate) usize);

/// Represents a deleted SSA value. Used as a placeholder.
/// This points to the original SSA value's ID.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct DeletedSSAValue(pub(crate) usize);

/// Represents a test SSA value. Used in tests only.
/// This SSAValue may not exist in the SSA database.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct TestSSAValue(pub usize);

/// Information about an SSA value in the database.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SSAInfo<L: Language> {
    pub(crate) id: SSAValue,
    pub(crate) name: Option<Symbol>,
    pub(crate) ty: L::TypeLattice,
    pub(crate) kind: SSAKind,
    pub(crate) uses: HashSet<Use>,
}

impl<L: Language> SSAInfo<L> {
    pub fn new(id: SSAValue, name: Option<Symbol>, ty: L::TypeLattice, kind: SSAKind) -> Self {
        Self {
            id,
            name,
            ty,
            kind,
            uses: HashSet::new(),
        }
    }
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct Use {
    stmt: StatementId,
    operand_index: usize,
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum SSAKind {
    Result(StatementId, usize),
    BlockArgument(Block, usize),
    Deleted,
    // should not appear in final SSA IR
    /// A placeholder for builders to update the Block information later.
    /// It holds the index of the argument in the block's argument list.
    BuilderBlockArgument(usize),
    /// A placeholder for builders to update the Result information later when building the statement.
    /// It holds the index of the result in the statement's result list.
    BuilderResult(usize),
    /// A placeholder for tests to create SSA values that do not exist in the SSA database.
    Test,
}

impl From<&SSAValue> for SSAValue {
    fn from(ssa: &SSAValue) -> Self {
        SSAValue(ssa.0)
    }
}

impl From<SSAValue> for ResultValue {
    fn from(ssa: SSAValue) -> Self {
        ResultValue(ssa.0)
    }
}

impl From<SSAValue> for BlockArgument {
    fn from(ssa: SSAValue) -> Self {
        BlockArgument(ssa.0)
    }
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
