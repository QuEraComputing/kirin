use crate::arena::{GetInfo, Id, Identifier};
use crate::identifier;
use crate::{Dialect, Symbol};
use smallvec::SmallVec;

use super::port::{Port, PortParent};
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
// Port has its own Display impl in port.rs

/// Represents a test SSA value. Used in tests only.
/// This SSAValue may not exist in the SSA database.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct TestSSAValue(pub usize);

/// Information about an SSA value in the database.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SSAInfo<L: Dialect> {
    pub(crate) id: SSAValue,
    pub(crate) name: Option<Symbol>,
    pub(crate) ty: L::Type,
    pub(crate) kind: BuilderSSAKind,
    pub(crate) uses: SmallVec<[Use; 2]>,
}

impl<L: Dialect> SSAInfo<L> {
    pub fn new(id: SSAValue, name: Option<Symbol>, ty: L::Type, kind: BuilderSSAKind) -> Self {
        Self {
            id,
            name,
            ty,
            kind,
            uses: SmallVec::new(),
        }
    }

    pub fn id(&self) -> SSAValue {
        self.id
    }

    pub fn name(&self) -> Option<Symbol> {
        self.name
    }

    pub fn ty(&self) -> &L::Type {
        &self.ty
    }

    pub fn set_ty(&mut self, ty: L::Type) {
        self.ty = ty;
    }

    /// Returns the clean [`SSAKind`] for this SSA value.
    ///
    /// # Panics
    ///
    /// Panics if the SSA kind is `Unresolved` or `Test`. This should only be
    /// called on finalized IR (after [`BuilderStageInfo::finalize`](crate::BuilderStageInfo::finalize)).
    pub fn kind(&self) -> SSAKind {
        self.kind.as_resolved().unwrap_or_else(|| {
            panic!(
                "SSAInfo::kind() called on unresolved SSA value {:?} (kind: {:?}). \
                 Use builder_kind() for build-time access.",
                self.id, self.kind
            )
        })
    }

    /// Returns the build-time [`BuilderSSAKind`] for this SSA value.
    ///
    /// Use this in builder code that may encounter `Unresolved` or `Test` variants.
    /// Downstream consumers should use [`kind()`](Self::kind) instead.
    pub fn builder_kind(&self) -> &BuilderSSAKind {
        &self.kind
    }

    pub fn uses(&self) -> &SmallVec<[Use; 2]> {
        &self.uses
    }

    pub fn uses_mut(&mut self) -> &mut SmallVec<[Use; 2]> {
        &mut self.uses
    }
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct Use {
    stmt: Statement,
    operand_index: usize,
}

/// A lookup key for builder placeholders — resolved at build time to the real SSA value.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum BuilderKey {
    /// Lookup by positional index.
    Index(usize),
    /// Lookup by name (interned symbol, matched against the builder's name declarations).
    Named(Symbol),
}

/// Build-time resolution info for [`BuilderSSAKind::Unresolved`] placeholders.
///
/// Each variant describes what the SSA value will eventually resolve to
/// once the enclosing builder (block, digraph, ungraph, statement) finalizes.
/// Must not appear in finalized IR.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ResolutionInfo {
    /// References a graph edge port by index or name.
    Port(BuilderKey),
    /// References a graph capture port by index or name.
    Capture(BuilderKey),
    /// References a block argument by index or name.
    BlockArgument(BuilderKey),
    /// A result SSA whose parent statement is not yet known.
    /// The `usize` is the result index in the statement's result list.
    Result(usize),
}

/// The kind of an SSA value in finalized IR.
///
/// After [`BuilderStageInfo::finalize`](crate::BuilderStageInfo::finalize),
/// every SSA value is guaranteed to have one of these three kinds.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum SSAKind {
    Result(Statement, usize),
    BlockArgument(Block, usize),
    Port(PortParent, usize),
}

/// Build-time SSA kind that includes unresolved placeholders.
///
/// Used internally during IR construction. Must be resolved to an [`SSAKind`]
/// variant before finalization. Use [`BuilderSSAKind::as_resolved`] to convert.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum BuilderSSAKind {
    Result(Statement, usize),
    BlockArgument(Block, usize),
    Port(PortParent, usize),
    /// Build-time placeholder — resolved by the appropriate builder/emitter.
    /// Must not appear in finalized IR.
    Unresolved(ResolutionInfo),
    /// A placeholder for tests to create SSA values that do not exist in the SSA database.
    Test,
}

impl BuilderSSAKind {
    /// Convert to a clean [`SSAKind`] if this is a resolved variant.
    ///
    /// Returns `None` for `Unresolved` and `Test` variants.
    pub fn as_resolved(&self) -> Option<SSAKind> {
        match *self {
            BuilderSSAKind::Result(stmt, idx) => Some(SSAKind::Result(stmt, idx)),
            BuilderSSAKind::BlockArgument(block, idx) => Some(SSAKind::BlockArgument(block, idx)),
            BuilderSSAKind::Port(parent, idx) => Some(SSAKind::Port(parent, idx)),
            BuilderSSAKind::Unresolved(_) | BuilderSSAKind::Test => None,
        }
    }
}

impl From<SSAKind> for BuilderSSAKind {
    fn from(kind: SSAKind) -> Self {
        match kind {
            SSAKind::Result(stmt, idx) => BuilderSSAKind::Result(stmt, idx),
            SSAKind::BlockArgument(block, idx) => BuilderSSAKind::BlockArgument(block, idx),
            SSAKind::Port(parent, idx) => BuilderSSAKind::Port(parent, idx),
        }
    }
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
impl_from_ssa!(Port);

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
impl_from_test!(Port);

impl<L: Dialect, T> GetInfo<L> for T
where
    T: Into<SSAValue> + Identifier,
{
    type Info = crate::arena::Item<SSAInfo<L>>;

    fn get_info<'a>(&self, stage: &'a crate::StageInfo<L>) -> Option<&'a Self::Info> {
        stage.ssas.get(*self)
    }

    fn get_info_mut<'a>(&self, stage: &'a mut crate::StageInfo<L>) -> Option<&'a mut Self::Info> {
        stage.ssas.get_mut(*self)
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

    #[test]
    fn test_ssa_value_display() {
        assert_eq!(format!("{}", SSAValue(Id(0))), "%0");
        assert_eq!(format!("{}", SSAValue(Id(42))), "%42");
    }

    #[test]
    fn test_result_value_display() {
        assert_eq!(format!("{}", ResultValue(Id(3))), "%3");
    }

    #[test]
    fn test_block_argument_display() {
        assert_eq!(format!("{}", BlockArgument(Id(7))), "%7");
    }

    #[test]
    fn test_ssa_roundtrip_through_result_value() {
        let rv = ResultValue(Id(42));
        let ssa: SSAValue = rv.into();
        let rv_back: ResultValue = ssa.into();
        assert_eq!(rv, rv_back);
    }

    #[test]
    fn test_ssa_roundtrip_through_block_argument() {
        let ba = BlockArgument(Id(10));
        let ssa: SSAValue = ba.into();
        let ba_back: BlockArgument = ssa.into();
        assert_eq!(ba, ba_back);
    }

    #[test]
    fn test_test_ssa_value_conversion() {
        let tsv = TestSSAValue(5);
        let ssa: SSAValue = tsv.into();
        let tsv_back: TestSSAValue = ssa.into();
        assert_eq!(tsv, tsv_back);
    }

    #[test]
    fn test_ssa_from_ref() {
        let ssa = SSAValue(Id(99));
        let ssa_copy: SSAValue = (&ssa).into();
        assert_eq!(ssa, ssa_copy);
    }
}
