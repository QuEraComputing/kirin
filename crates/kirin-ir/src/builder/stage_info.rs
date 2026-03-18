use std::fmt;
use std::ops::{Deref, DerefMut};

use crate::node::ssa::{SSAKind, SSAValue};
use crate::{Dialect, StageInfo};

/// Error returned by [`BuilderStageInfo::finalize`] when build-time SSAs
/// have not been resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FinalizeError {
    /// An `SSAKind::Unresolved` SSA was found — the builder did not resolve
    /// all placeholder references.
    UnresolvedSSA(SSAValue),
    /// An `SSAKind::Test` SSA was found — test-only SSAs must not appear in
    /// finalized IR.
    TestSSA(SSAValue),
}

impl fmt::Display for FinalizeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FinalizeError::UnresolvedSSA(ssa) => {
                write!(f, "unresolved SSA value {ssa} in finalized IR")
            }
            FinalizeError::TestSSA(ssa) => {
                write!(f, "test SSA value {ssa} in finalized IR")
            }
        }
    }
}

impl std::error::Error for FinalizeError {}

/// A mutable builder wrapper around [`StageInfo`].
///
/// `BuilderStageInfo` provides the builder API surface for constructing IR:
/// creating SSA values, statements, blocks, regions, graphs, staged functions,
/// and specializations. The inner [`StageInfo`] is accessible via [`Deref`] and
/// [`DerefMut`] for read/write access to the underlying arenas and builder
/// methods.
///
/// Call [`finalize`](BuilderStageInfo::finalize) to validate the IR and obtain
/// the underlying `StageInfo`. Use [`into_inner`](BuilderStageInfo::into_inner)
/// to skip validation (escape hatch for tests and intermediate transforms).
pub struct BuilderStageInfo<L: Dialect>(pub(crate) StageInfo<L>);

impl<L: Dialect> Default for BuilderStageInfo<L> {
    fn default() -> Self {
        Self(StageInfo::default())
    }
}

impl<L: Dialect> fmt::Debug for BuilderStageInfo<L> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("BuilderStageInfo").field(&self.0).finish()
    }
}

impl<L: Dialect> Deref for BuilderStageInfo<L> {
    type Target = StageInfo<L>;
    fn deref(&self) -> &StageInfo<L> {
        &self.0
    }
}

impl<L: Dialect> DerefMut for BuilderStageInfo<L> {
    fn deref_mut(&mut self) -> &mut StageInfo<L> {
        &mut self.0
    }
}

impl<L: Dialect> From<StageInfo<L>> for BuilderStageInfo<L> {
    fn from(stage: StageInfo<L>) -> Self {
        Self(stage)
    }
}

impl<L: Dialect> BuilderStageInfo<L> {
    /// Validate the IR and return the underlying [`StageInfo`].
    ///
    /// Checks that no `SSAKind::Unresolved` or `SSAKind::Test` values remain.
    pub fn finalize(self) -> Result<StageInfo<L>, FinalizeError> {
        for ssa_info in self.0.ssas.iter() {
            match ssa_info.kind {
                SSAKind::Unresolved(_) => {
                    return Err(FinalizeError::UnresolvedSSA(ssa_info.id()));
                }
                SSAKind::Test => {
                    return Err(FinalizeError::TestSSA(ssa_info.id()));
                }
                _ => {}
            }
        }
        Ok(self.0)
    }

    /// Return the underlying [`StageInfo`] without validation.
    ///
    /// This is an escape hatch for tests and intermediate transforms that
    /// do not require finalization guarantees.
    pub fn into_inner(self) -> StageInfo<L> {
        self.0
    }
}
