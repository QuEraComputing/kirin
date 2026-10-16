use crate::arena::GetInfo;
use crate::{DiGraph, Dialect, SSAKind, SSAValue, Statement};

use crate::rewrite::rewriter::Rewriter;

/// Operand-edge bookkeeping shared by the `DiGraph` surgery methods.
impl<'a, L: Dialect> Rewriter<'a, L> {
    /// The member of `graph` that defines `value`, if `value` is a result of
    /// one. Only such values have an edge in `graph`'s petgraph.
    pub(in crate::rewrite) fn member_producer(
        &self,
        graph: DiGraph,
        value: SSAValue,
    ) -> Option<Statement> {
        let SSAKind::Result(producer, _) = *value.get_info(self.stage)?.kind() else {
            return None;
        };
        let info = graph.get_info(self.stage)?;
        info.node_idx_of(producer).map(|_| producer)
    }

    /// Mirror a new operand slot of member `consumer` reading `value`.
    /// Returns whether an edge was added.
    pub(in crate::rewrite) fn add_operand_edge(
        &mut self,
        graph: DiGraph,
        consumer: Statement,
        value: SSAValue,
    ) -> bool {
        let Some(producer) = self.member_producer(graph, value) else {
            return false;
        };
        graph
            .expect_info_mut(self.stage)
            .add_edge(producer, consumer, value);
        true
    }

    /// Drop the mirror of one operand slot of member `consumer` reading
    /// `value`. Returns whether an edge was removed.
    pub(in crate::rewrite) fn remove_operand_edge(
        &mut self,
        graph: DiGraph,
        consumer: Statement,
        value: SSAValue,
    ) -> bool {
        let Some(producer) = self.member_producer(graph, value) else {
            return false;
        };
        let removed = graph
            .expect_info_mut(self.stage)
            .remove_edge(producer, consumer, value);
        debug_assert!(removed, "no {value} edge into {consumer:?} to remove");
        removed
    }

    /// Mirror one operand slot of member `consumer` changing from `old` to
    /// `new`. Returns whether the topology changed.
    pub(in crate::rewrite) fn retarget_operand_edge(
        &mut self,
        graph: DiGraph,
        consumer: Statement,
        old: SSAValue,
        new: SSAValue,
    ) -> bool {
        let removed = self.remove_operand_edge(graph, consumer, old);
        let added = self.add_operand_edge(graph, consumer, new);
        removed || added
    }
}
