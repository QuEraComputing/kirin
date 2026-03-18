use std::ops::{Deref, DerefMut};

use crate::arena::Arena;
use crate::node::ssa::SSAInfo;
use crate::node_arenas::NodeArenas;
use crate::{BuilderStageInfo, Dialect, node::*};

/// The core stage info type for finalized IR.
///
/// After [`BuilderStageInfo::finalize`](crate::BuilderStageInfo::finalize),
/// every SSA value is guaranteed to have a type and a resolved kind.
/// The SSA arena holds clean [`SSAInfo`] values with `L::Type` and [`SSAKind`].
#[derive(Debug)]
pub struct StageInfo<L: Dialect> {
    pub(crate) nodes: NodeArenas<L>,
    pub(crate) ssas: Arena<SSAValue, SSAInfo<L>>,
}

impl<L> Default for StageInfo<L>
where
    L: Dialect,
{
    fn default() -> Self {
        Self {
            nodes: NodeArenas::default(),
            ssas: Arena::default(),
        }
    }
}

impl<L> Clone for StageInfo<L>
where
    L: Dialect,
    StatementInfo<L>: Clone,
    SSAInfo<L>: Clone,
{
    fn clone(&self) -> Self {
        Self {
            nodes: self.nodes.clone(),
            ssas: self.ssas.clone(),
        }
    }
}

impl<L: Dialect> Deref for StageInfo<L> {
    type Target = NodeArenas<L>;

    fn deref(&self) -> &Self::Target {
        &self.nodes
    }
}

impl<L: Dialect> DerefMut for StageInfo<L> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.nodes
    }
}

impl<L: Dialect> StageInfo<L> {
    /// Get a reference to the SSA values arena.
    pub fn ssa_arena(&self) -> &Arena<SSAValue, SSAInfo<L>> {
        &self.ssas
    }

    /// Temporarily convert this `StageInfo` into a [`BuilderStageInfo`], run
    /// the closure, then convert back.
    ///
    /// This is the bridge for code paths (e.g. [`Pipeline`](crate::Pipeline))
    /// that hold `&mut StageInfo<L>` via trait dispatch but need builder methods.
    pub fn with_builder<R>(&mut self, f: impl FnOnce(&mut BuilderStageInfo<L>) -> R) -> R {
        let stage = std::mem::take(self);
        let mut builder = BuilderStageInfo::from(stage);
        let result = f(&mut builder);
        *self = builder.finalize_unchecked();
        result
    }
}
