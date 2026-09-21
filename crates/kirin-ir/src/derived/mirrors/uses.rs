use smallvec::SmallVec;

use crate::arena::Id;
use crate::node::ssa::Use;
use crate::{DiGraph, Dialect, SSAValue, StageInfo, Statement};

use crate::derived::SlotMap;
use crate::derived::compare::multiset_eq;
use crate::derived::error::{Finding, Mismatch};

/// The expected contents of every [`SSAInfo::uses`](crate::SSAInfo) list
/// accumulated into one map.
pub(in crate::derived) type UseMap = SlotMap<SSAValue, SmallVec<[Use; 2]>>;

/// Compute the uses map from authoritative IR operations' operand and yield slots.
pub(in crate::derived) fn derive<L: Dialect>(
    stage: &StageInfo<L>,
    findings: &mut Vec<Finding>,
) -> UseMap {
    let mut table = UseMap::sized_like(&stage.ssas);

    // Collect the statement arguments as uses
    for (raw, item) in stage.nodes.statements.items.iter().enumerate() {
        if item.deleted() {
            continue;
        }

        let stmt = Statement::from(Id(raw));
        for (arg_index, &value) in item.definition.arguments().enumerate() {
            match live_entry(stage, &mut table, value) {
                Some(uses) => uses.push(Use::StatementOperand {
                    stmt,
                    index: arg_index,
                }),
                None => findings.push(Finding::DanglingOperand {
                    stmt,
                    index: arg_index,
                    value,
                }),
            }
        }
    }

    // Collect the digraph yields as uses
    for (raw, item) in stage.nodes.digraphs.items.iter().enumerate() {
        if item.deleted() {
            continue;
        }

        let digraph = DiGraph::from(Id(raw));
        for (yield_index, &value) in item.yields().iter().enumerate() {
            match live_entry(stage, &mut table, value) {
                Some(uses) => uses.push(Use::DiGraphYield {
                    graph: digraph,
                    index: yield_index,
                }),
                None => findings.push(Finding::DanglingYield {
                    graph: digraph,
                    index: yield_index,
                    value,
                }),
            }
        }
    }

    table
}

impl UseMap {
    /// Write the derived index into the stage. Finalization only.
    pub(in crate::derived) fn install<L: Dialect>(self, stage: &mut StageInfo<L>) {
        assert!(self.len() == stage.ssas.len());
        for (item, uses) in stage.ssas.items.iter_mut().zip(self.into_iter()) {
            if let Some(info) = item.as_mut() {
                *info.uses_mut() = uses;
            }
        }
    }

    /// Compare the installed lists with this derivation, appending one
    /// [`Mismatch`] per disagreeing value. Never writes to the stage.
    pub(in crate::derived) fn verify<L: Dialect>(
        &self,
        stage: &StageInfo<L>,
        out: &mut Vec<Mismatch>,
    ) {
        for (raw, item) in stage.ssas.items.iter().enumerate() {
            // A tombstoned slot keeps whatever use list it had when it was
            // deleted; that payload is unreachable, so it is not compared.
            if item.deleted() {
                continue;
            }
            let Some(info) = item.data.as_ref() else {
                continue;
            };
            let value = SSAValue::from(Id(raw));
            let derived: &[Use] = self.get(value).map_or(&[], |uses| uses.as_slice());
            if !multiset_eq(info.uses(), derived) {
                out.push(Mismatch::Uses {
                    value,
                    installed: info.uses().to_vec(),
                    derived: derived.to_vec(),
                });
            }
        }
    }
}

/// The table entry for `value`, or `None` if `value` is not a live SSA value.
///
/// Liveness is authoritative in `stage`. The table is sized from the same
/// arena, so a live id always has an entry.
fn live_entry<'t, L: Dialect>(
    stage: &StageInfo<L>,
    table: &'t mut UseMap,
    value: SSAValue,
) -> Option<&'t mut SmallVec<[Use; 2]>> {
    let item = stage.ssas.get(value)?;
    if item.deleted() || item.data.is_none() {
        return None;
    }
    table.get_mut(value)
}
