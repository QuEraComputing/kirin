use std::collections::HashSet;

use smallvec::SmallVec;

use crate::arena::Id;
use crate::{Block, Dialect, StageInfo, Statement, StatementParent};

use crate::derived::SlotMap;
use crate::derived::compare::multiset_eq;
use crate::derived::error::{Finding, Mismatch};

/// The expected contents of every [`BlockInfo::predecessors`](crate::BlockInfo)
/// list indexed by the same stage's arena `Block` indices.
pub(in crate::derived) type PredecessorMap = SlotMap<Block, SmallVec<[Block; 4]>>;

/// Compute the reverse control-flow index from authoritative successor
/// references.
///
/// Successor references carried by statements are the authoritative forward
/// edges. A statement contributes an edge only when its structural parent is a
/// block, because the predecessor relation is between blocks.
pub(in crate::derived) fn derive<L: Dialect>(
    stage: &StageInfo<L>,
    findings: &mut Vec<Finding>,
) -> PredecessorMap {
    let mut table = PredecessorMap::sized_like(&stage.nodes.blocks);
    let mut seen: HashSet<(Block, Block)> = HashSet::new();

    for (raw, item) in stage.nodes.statements.items.iter().enumerate() {
        if item.deleted() {
            continue;
        }
        let Some(StatementParent::Block(source)) = item.parent else {
            continue;
        };
        let stmt = Statement::from(Id(raw));

        for successor in item.definition.successors() {
            let target = successor.target();
            let Some(predecessors) = live_entry(stage, &mut table, target) else {
                findings.push(Finding::DanglingSuccessor { stmt, target });
                continue;
            };

            // Multiple successor edges from one source block to the same target still
            // represent one predecessor, so `(source, target)` pairs are recorded once.
            if seen.insert((source, target)) {
                predecessors.push(source);
            }
        }
    }

    table
}

impl PredecessorMap {
    /// Write the derived index into the stage. Finalization only.
    pub(in crate::derived) fn install<L: Dialect>(self, stage: &mut StageInfo<L>) {
        let stage_blocks = stage.nodes.blocks.items.iter_mut();
        for (item, predecessors) in stage_blocks.zip(self.into_iter()) {
            item.predecessors = predecessors;
        }
    }

    /// Compare the installed lists with this derivation, appending one
    /// [`Mismatch`] per disagreeing block. Never writes to the stage.
    pub(in crate::derived) fn verify<L: Dialect>(
        &self,
        stage: &StageInfo<L>,
        out: &mut Vec<Mismatch>,
    ) {
        for (raw, item) in stage.nodes.blocks.items.iter().enumerate() {
            if item.deleted() {
                continue;
            }
            let block = Block::from(Id(raw));
            let derived: &[Block] = self.get(block).map_or(&[], |blocks| blocks.as_slice());
            if !multiset_eq(&item.predecessors, derived) {
                out.push(Mismatch::Predecessors {
                    block,
                    installed: item.predecessors.to_vec(),
                    derived: derived.to_vec(),
                });
            }
        }
    }
}

/// Gets the table entry for `block`, or `None` if `block` is not a live block based on the stage's arena.
fn live_entry<'t, L: Dialect>(
    stage: &StageInfo<L>,
    table: &'t mut PredecessorMap,
    block: Block,
) -> Option<&'t mut SmallVec<[Block; 4]>> {
    let item = stage.nodes.blocks.get(block)?;
    if item.deleted() {
        return None;
    }
    table.get_mut(block)
}
