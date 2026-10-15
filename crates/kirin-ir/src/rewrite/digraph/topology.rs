use crate::arena::{GetInfo, GetInfoMut};
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
    pub(in crate::rewrite) fn add_operand_edge(
        &mut self,
        graph: DiGraph,
        consumer: Statement,
        value: SSAValue,
    ) {
        if let Some(producer) = self.member_producer(graph, value) {
            graph
                .get_info_mut(self.stage)
                .expect("graph is live in this stage")
                .add_edge(producer, consumer, value);
        }
    }

    /// Drop the mirror of one operand slot of member `consumer` reading
    /// `value`.
    #[expect(dead_code, reason = "wired up by operand edits on DiGraph members")]
    pub(in crate::rewrite) fn remove_operand_edge(
        &mut self,
        graph: DiGraph,
        consumer: Statement,
        value: SSAValue,
    ) {
        if let Some(producer) = self.member_producer(graph, value) {
            let removed = graph
                .get_info_mut(self.stage)
                .expect("graph is live in this stage")
                .remove_edge(producer, consumer, value);
            debug_assert!(removed, "no {value} edge into {consumer:?} to remove");
        }
    }
}
