use indexmap::IndexMap;
use rustc_hash::FxBuildHasher;
use smallvec::SmallVec;

use crate::arena::{GetInfo, GetInfoMut};
use crate::node::port::PortParent;
use crate::{Dialect, SSAKind, SSAValue, Statement, StatementParent, UnGraph, Use};

use crate::rewrite::rewriter::Rewriter;

/// The node members reading one edge value, one entry per operand slot.
pub(in crate::rewrite) type Incidences = SmallVec<[Statement; 2]>;

/// Wire → its incidences before an edit, for [`Rewriter::resync_wires`].
pub(in crate::rewrite) type WireSnapshot = IndexMap<SSAValue, Incidences, FxBuildHasher>;

/// Incidence bookkeeping shared by the `UnGraph` surgery methods.
impl<'a, L: Dialect> Rewriter<'a, L> {
    /// Whether `value` is an edge value of `graph`: a result of one of its
    /// edge statements, or one of its boundary ports.
    pub(in crate::rewrite) fn is_edge_value(&self, graph: UnGraph, value: SSAValue) -> bool {
        let Some(info) = value.get_info(self.stage) else {
            return false;
        };
        match *info.kind() {
            SSAKind::Result(stmt, _) => stmt.get_info(self.stage).is_some_and(|item| {
                item.parent == Some(StatementParent::UnGraph(graph)) && item.definition.is_edge()
            }),
            SSAKind::Port(PortParent::UnGraph(owner), index) => {
                owner == graph
                    && graph
                        .get_info(self.stage)
                        .is_some_and(|item| index < item.edge_count())
            }
            _ => false,
        }
    }

    /// The node members of `graph` reading `value`, one entry per operand
    /// slot (a node reading it twice appears twice).
    pub(in crate::rewrite) fn incidences(&self, graph: UnGraph, value: SSAValue) -> Incidences {
        let (Some(info), Some(graph_info)) =
            (value.get_info(self.stage), graph.get_info(self.stage))
        else {
            return Incidences::new();
        };
        info.uses()
            .iter()
            .filter_map(|site| match *site {
                Use::StatementOperand { stmt, .. } => Some(stmt),
                Use::DiGraphYield { .. } => None,
            })
            .filter(|&stmt| graph_info.node_idx_of(stmt).is_some())
            .collect()
    }

    /// Snapshot the wires of `graph` among `values` (each once; values that
    /// are not wires of `graph` are skipped) before an edit that may change
    /// their incidences.
    pub(in crate::rewrite) fn snapshot_wires(
        &self,
        graph: UnGraph,
        values: impl IntoIterator<Item = SSAValue>,
    ) -> WireSnapshot {
        let mut wires = WireSnapshot::default();
        for value in values {
            if !wires.contains_key(&value) && self.is_edge_value(graph, value) {
                wires.insert(value, self.incidences(graph, value));
            }
        }
        wires
    }

    /// After the edit, bring the edge of each snapshotted wire in line with
    /// its incidences now. Returns whether the petgraph changed.
    pub(in crate::rewrite) fn resync_wires(&mut self, graph: UnGraph, wires: WireSnapshot) -> bool {
        let mut changed = false;
        for (value, orig_incidences) in wires {
            let after = self.incidences(graph, value);
            changed |= self.resync_edge(graph, value, &orig_incidences, &after);
        }
        changed
    }

    /// Bring `graph`'s petgraph edge for `value` in line after its incidences
    /// changed from `before` to `after`: an edge exists exactly when there are
    /// two (a wire with more has none; that is invalid IR, which
    /// [`verify_derived`](crate::verify_derived) reports). Returns whether the
    /// petgraph changed.
    fn resync_edge(
        &mut self,
        graph: UnGraph,
        value: SSAValue,
        before: &[Statement],
        after: &[Statement],
    ) -> bool {
        if same_endpoints(before, after) {
            return false;
        }
        let info = graph
            .get_info_mut(self.stage)
            .expect("graph is live in this stage");
        let mut changed = false;
        if let &[a, b] = before {
            changed |= info.remove_edge(a, b, value);
        }
        if let &[a, b] = after {
            info.add_edge(a, b, value);
            changed = true;
        }
        changed
    }
}

/// Whether two incidence lists describe the same edge (or the same absence of
/// one): equal as unordered pairs, or both without an edge.
fn same_endpoints(before: &[Statement], after: &[Statement]) -> bool {
    match (before, after) {
        (&[a, b], &[c, d]) => (a == c && b == d) || (a == d && b == c),
        (before, after) => before.len() != 2 && after.len() != 2,
    }
}
