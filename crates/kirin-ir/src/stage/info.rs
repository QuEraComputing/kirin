use std::{
    collections::HashSet,
    ops::{Deref, DerefMut},
};

use crate::arena::{Arena, Id};
use crate::node::ssa::{SSAInfo, Use};
use crate::{BuilderStageInfo, Dialect, node::*};

use super::arenas::Arenas;

/// Finalized IR for a single compilation stage.
///
/// `StageInfo` holds the node arenas (blocks, statements, cfgs, graphs,
/// functions) and a clean SSA arena where every value has a resolved type and
/// kind. It is the read-only output of [`BuilderStageInfo::finalize`].
///
/// # SSA tombstones
///
/// The SSA arena stores `Option<SSAInfo<L>>`: live items are `Some(info)`,
/// deleted (tombstoned) items are `None`. This avoids the need for `unsafe`
/// zeroed memory and guarantees that stale IDs pointing at deleted slots
/// never yield invalid data — they yield `None` instead.
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
///     let sf = b.staged_function().name(func_name).new().unwrap();
///
///     let arg = b.block_argument().index(0);
///     let ret = b.statement().definition(MyDialect::Return(arg)).new();
///     let block = b.block().argument(MyType::I64).terminator(ret).new();
///     let cfg = b.cfg().add_block(block).new();
///     let body = b.statement().definition(MyDialect::FuncBody(cfg)).new();
///
///     b.specialize().staged_func(sf).body(body).new().unwrap();
/// });
/// // stage is back to StageInfo with the new function added
/// ```
#[derive(Debug)]
pub struct StageInfo<L: Dialect> {
    pub(crate) nodes: Arenas<L>,
    pub(crate) ssas: Arena<SSAValue, Option<SSAInfo<L>>>,
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
    Option<SSAInfo<L>>: Clone,
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
    ///
    /// The arena stores `Option<SSAInfo<L>>`: live items are `Some(info)`,
    /// deleted (tombstoned) items are `None`.
    pub fn ssa_arena(&self) -> &Arena<SSAValue, Option<SSAInfo<L>>> {
        &self.ssas
    }

    /// Rebuild the def-use index ([`SSAInfo::uses`](crate::SSAInfo)) from the
    /// authoritative storage.
    ///
    /// Clears every live value's use list, then records one [`Use`](crate::Use)
    /// per position that reads a value:
    ///
    /// - each live statement's operands (in [`HasArguments`](crate::HasArguments)
    ///   order) → [`Use::StatementOperand`](crate::Use), and
    /// - each live `DiGraph` body's yields (in yield order) →
    ///   [`Use::DiGraphYield`](crate::Use). A yield is a boundary export with no
    ///   backing statement, so it would otherwise be invisible to the operand
    ///   scan.
    ///
    /// The operand and yield slots are the ground truth; this list is a derived
    /// reverse index over them. `UnGraph` contributes nothing: its edges are
    /// statements whose operands are already covered above; graph ports and
    /// block arguments are definitions, not uses.
    ///
    /// Idempotent — safe to re-run. Called at
    /// [`finalize`](crate::BuilderStageInfo::finalize) so finalized IR ships a
    /// populated index; a mutation layer must call it (or maintain the index
    /// incrementally) after changing operands or yields.
    pub fn rebuild_use_index(&mut self) {
        let StageInfo { nodes, ssas } = self;

        for item in ssas.iter_mut() {
            if let Some(info) = (**item).as_mut() {
                info.uses_mut().clear();
            }
        }

        for (raw, item) in nodes.statements.items.iter().enumerate() {
            if item.deleted() {
                continue;
            }
            let stmt = Statement(Id(raw));
            let operands: Vec<SSAValue> = item.data.definition.arguments().copied().collect();
            for (index, operand) in operands.into_iter().enumerate() {
                if let Some(slot) = ssas.get_mut(operand)
                    && let Some(info) = (**slot).as_mut()
                {
                    info.uses_mut().push(Use::StatementOperand { stmt, index });
                }
            }
        }

        for (raw, item) in nodes.digraphs.items.iter().enumerate() {
            if item.deleted() {
                continue;
            }
            let graph = DiGraph::from(Id(raw));
            let yields: Vec<SSAValue> = item.data.yields().to_vec();
            for (index, yielded) in yields.into_iter().enumerate() {
                if let Some(slot) = ssas.get_mut(yielded)
                    && let Some(info) = (**slot).as_mut()
                {
                    info.uses_mut().push(Use::DiGraphYield { graph, index });
                }
            }
        }
    }

    /// Rebuild the reverse control-flow index stored in
    /// [`BlockInfo::predecessors`](crate::BlockInfo::predecessors).
    ///
    /// Successor references on statements are the authoritative forward
    /// edges. This method clears every live block's cached predecessors, then
    /// scans each live statement whose structural parent is a block. For every
    /// successor target, the source block is recorded as a predecessor of that
    /// target.
    ///
    /// Multiple successor edges from one source block to the same target still
    /// represent one predecessor block, so duplicate `(source, target)` pairs
    /// are recorded only once. Blocks directly owned by statements do not need
    /// synthetic predecessor entries: backward traversal reaches their owner
    /// through [`BlockParent::Statement`](crate::BlockParent::Statement).
    ///
    /// Idempotent — safe to re-run after successor edges change. Called during
    /// finalization so finalized IR ships with a populated reverse index.
    pub fn rebuild_predecessor_index(&mut self) {
        let StageInfo { nodes, .. } = self;
        let (blocks, statements) = (&mut nodes.blocks, &nodes.statements);

        for block in blocks.iter_mut() {
            block.predecessors.clear();
        }

        let mut seen = HashSet::new();
        for statement in statements.iter() {
            let Some(StatementParent::Block(source)) = statement.parent else {
                continue;
            };

            for successor in statement.definition.successors() {
                let target = successor.target();
                if !seen.insert((source, target)) {
                    continue;
                }

                let Some(target_info) = blocks.get_mut(target) else {
                    continue;
                };
                if !target_info.deleted() {
                    target_info.predecessors.push(source);
                }
            }
        }
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
