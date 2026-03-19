use std::ops::{Deref, DerefMut};

use crate::arena::Arena;
use crate::node::ssa::SSAInfo;
use crate::{BuilderStageInfo, Dialect, node::*};

use super::arenas::Arenas;

/// Finalized IR for a single compilation stage.
///
/// `StageInfo` holds the node arenas (blocks, statements, regions, graphs,
/// functions) and a clean SSA arena where every value has a resolved type and
/// kind. It is the read-only output of [`BuilderStageInfo::finalize`].
///
/// # Obtaining a `StageInfo`
///
/// Build IR with [`BuilderStageInfo`], then call
/// [`finalize()`](crate::BuilderStageInfo::finalize):
///
/// ```ignore
/// let mut builder = BuilderStageInfo::<MyDialect>::default();
/// // ... construct IR ...
/// let stage: StageInfo<MyDialect> = builder.finalize().unwrap();
/// ```
///
/// # Querying
///
/// Use [`GetInfo`](crate::GetInfo) to look up node info by ID:
///
/// ```ignore
/// let block_info = block.expect_info(&stage);
/// let ssa_info = ssa_value.expect_info(&stage);   // SSAInfo with ty: L::Type
/// let stmts: Vec<_> = block.statements(&stage).collect();
/// ```
///
/// # Constructing new functions on an existing `StageInfo`
///
/// When working through a [`Pipeline`](crate::Pipeline), stages are stored as
/// `StageInfo`. To add new functions, use [`with_builder`](Self::with_builder)
/// which temporarily converts to a [`BuilderStageInfo`]:
///
/// ```ignore
/// let stage: &mut StageInfo<MyDialect> = pipeline.stage_mut(stage_id).unwrap();
/// stage.with_builder(|b| {
///     // b is &mut BuilderStageInfo<MyDialect>
///     let sf = b.staged_function(Some(func_name), None, None, None).unwrap();
///
///     let arg = b.block_argument().index(0);
///     let ret = b.statement(MyDialect::Return(arg));
///     let block = b.block().argument(MyType::I64).terminator(ret).new();
///     let region = b.region().add_block(block).new();
///     let body = b.statement(MyDialect::FuncBody(region));
///
///     b.specialize(sf, None, body, None).unwrap();
/// });
/// // stage is back to StageInfo with the new function added
/// ```
#[derive(Debug)]
pub struct StageInfo<L: Dialect> {
    pub(crate) nodes: Arenas<L>,
    pub(crate) ssas: Arena<SSAValue, SSAInfo<L>>,
}

impl<L> Default for StageInfo<L>
where
    L: Dialect,
{
    fn default() -> Self {
        Self {
            nodes: Arenas::default(),
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
    type Target = Arenas<L>;

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

    /// Temporarily convert to a [`BuilderStageInfo`] for construction, then
    /// convert back.
    ///
    /// This is the bridge for code that holds `&mut StageInfo<L>` (e.g. via
    /// [`Pipeline::stage_mut`](crate::Pipeline::stage_mut) or
    /// [`HasStageInfo`](crate::HasStageInfo)) but needs builder methods.
    ///
    /// The SSA arena is converted in each direction (O(n)), so prefer
    /// batching construction inside a single `with_builder` call rather than
    /// calling it per-statement.
    pub fn with_builder<R>(&mut self, f: impl FnOnce(&mut BuilderStageInfo<L>) -> R) -> R {
        let stage = std::mem::take(self);
        let mut builder = BuilderStageInfo::from(stage);
        let result = f(&mut builder);
        *self = builder.finalize_unchecked();
        result
    }
}
