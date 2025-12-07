use crate::arena::{GetInfo, Id, Identifier};
use crate::identifier;
use crate::{Dialect, Symbol};
use std::collections::HashSet;

use super::{block::Block, stmt::Statement};

identifier! {
    /// Represents a general SSA value that can be either
    /// a value produced by a statement or an argument to a block.

    /// If you are certain about the kind of SSA value, consider using
    /// `ResultValue` or `BlockArgument` instead.
    struct SSAValue
}

identifier! {
    /// Represents a value produced by a statement.
    struct ResultValue
}

identifier! {
    /// Represents an argument to a block.
    struct BlockArgument
}

identifier! {
    /// Represents a deleted SSA value. Used as a placeholder.
    ///
    /// This points to the original SSA value's ID.
    struct DeletedSSAValue
}

macro_rules! impl_ssa_display {
    ($name:ident) => {
        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "%{}", self.0.raw())
            }
        }
    };
}

impl_ssa_display!(SSAValue);
impl_ssa_display!(ResultValue);
impl_ssa_display!(BlockArgument);
impl_ssa_display!(DeletedSSAValue);

/// Represents a test SSA value. Used in tests only.
/// This SSAValue may not exist in the SSA database.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct TestSSAValue(pub usize);

/// Information about an SSA value in the database.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SSAInfo<L: Dialect> {
    pub(crate) id: SSAValue,
    pub(crate) name: Option<Symbol>,
    pub(crate) ty: L::TypeLattice,
    pub(crate) kind: SSAKind,
    pub(crate) uses: HashSet<Use>,
}

impl<L: Dialect> SSAInfo<L> {
    pub fn new(id: SSAValue, name: Option<Symbol>, ty: L::TypeLattice, kind: SSAKind) -> Self {
        Self {
            id,
            name,
            ty,
            kind,
            uses: HashSet::new(),
        }
    }

    pub fn name(&self) -> Option<Symbol> {
        self.name
    }

    pub fn ty(&self) -> &L::TypeLattice {
        &self.ty
    }

    pub fn set_ty(&mut self, ty: L::TypeLattice) {
        self.ty = ty;
    }

    pub fn kind(&self) -> &SSAKind {
        &self.kind
    }

    pub fn uses(&self) -> &HashSet<Use> {
        &self.uses
    }

    pub fn uses_mut(&mut self) -> &mut HashSet<Use> {
        &mut self.uses
    }
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct Use {
    stmt: Statement,
    operand_index: usize,
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum SSAKind {
    Result(Statement, usize),
    BlockArgument(Block, usize),
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

impl From<TestSSAValue> for Id {
    fn from(value: TestSSAValue) -> Self {
        Id(value.0)
    }
}

macro_rules! impl_from_ssa {
    ($name:ident) => {
        impl From<SSAValue> for $name {
            fn from(ssa: SSAValue) -> Self {
                $name(ssa.into())
            }
        }

        impl From<$name> for SSAValue {
            fn from(value: $name) -> Self {
                SSAValue(value.into())
            }
        }
    };
}

impl_from_ssa!(ResultValue);
impl_from_ssa!(BlockArgument);
impl_from_ssa!(DeletedSSAValue);

impl From<SSAValue> for TestSSAValue {
    fn from(ssa: SSAValue) -> Self {
        TestSSAValue(ssa.0.raw())
    }
}

impl From<TestSSAValue> for SSAValue {
    fn from(value: TestSSAValue) -> Self {
        SSAValue(value.into())
    }
}

impl From<&SSAValue> for SSAValue {
    fn from(ssa: &SSAValue) -> Self {
        SSAValue(ssa.0)
    }
}

macro_rules! impl_from_test {
    ($name:ident) => {
        impl From<TestSSAValue> for $name {
            fn from(tsv: TestSSAValue) -> Self {
                $name(tsv.into())
            }
        }
    };
}

impl_from_test!(ResultValue);
impl_from_test!(BlockArgument);
impl_from_test!(DeletedSSAValue);

impl<L: Dialect, T> GetInfo<L> for T
where
    T: Into<SSAValue> + Identifier,
{
    type Info = crate::arena::Item<SSAInfo<L>>;

    fn get_info<'a>(&self, context: &'a crate::Context<L>) -> Option<&'a Self::Info> {
        context.ssas.get(*self)
    }

    fn get_info_mut<'a>(&self, context: &'a mut crate::Context<L>) -> Option<&'a mut Self::Info> {
        context.ssas.get_mut(*self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ssa_value_conversion() {
        let rv = ResultValue(Id(42));
        let ba = BlockArgument(Id(84));

        let ssa_from_rv: SSAValue = rv.into();
        let ssa_from_ba: SSAValue = ba.into();

        assert_eq!(ssa_from_rv, SSAValue(Id(42)));
        assert_eq!(ssa_from_ba, SSAValue(Id(84)));
    }
}
