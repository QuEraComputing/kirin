use crate::arena::{GetInfo, GetInfoMut};
use crate::{Dialect, SSAValue, Statement, UnGraph, UnGraphInfo};

use crate::rewrite::rewriter::{MutationEvent, RewriteError, Rewriter};

/// Erasing and tombstoning `UnGraph` members.
impl<'a, L: Dialect> Rewriter<'a, L> {
    /// The `UnGraph` arm of [`Rewriter::erase_statement`]: `stmt` is a live
    /// node or edge statement of `graph`.
    pub(in crate::rewrite) fn erase_ungraph_member(
        &mut self,
        stmt: Statement,
        graph: UnGraph,
    ) -> Result<(), RewriteError> {
        // For an edge statement, this means no node reads its wires.
        self.ensure_results_unused(stmt)?;

        let is_node = graph.expect_info(self.stage).node_idx_of(stmt).is_some();
        if is_node {
            // A node: each wire it read loses its endpoints. The edge changes
            // only when the count crosses 2: 2 → 1 (or 2 → 0 for a self-loop)
            // removes it, 3 → 2 restores it; other counts change nothing.
            // Drop its operand uses, resync those wires while its node still
            // exists, then remove it.
            let operands: Vec<SSAValue> = stmt
                .expect_info(self.stage)
                .definition
                .arguments()
                .copied()
                .collect();
            let wires = self.snapshot_wires(graph, operands);
            self.tombstone_statement(stmt, None);
            self.resync_wires(graph, wires);
            graph
                .get_info_mut(self.stage)
                .expect("graph is live in this stage")
                .remove_node(stmt);
        } else {
            // An edge statement: no node reads its wires (checked above), so
            // they have no edges; just drop it from the members.
            graph
                .get_info_mut(self.stage)
                .expect("graph is live in this stage")
                .remove_edge_member(stmt);
            self.tombstone_statement(stmt, None);
        }

        self.events.push(MutationEvent::ErasedStatement { stmt });
        self.events
            .push(MutationEvent::ChangedUnGraphTopology { graph });
        Ok(())
    }

    /// Tombstone `graph` with everything in it: its nodes and edge statements
    /// (recursing into their nested bodies) with their uses and results, and
    /// its ports. The graph's own topology is left as is.
    pub(in crate::rewrite) fn tombstone_ungraph(&mut self, graph: UnGraph) {
        let Ok(info) = self.live_ungraph(graph) else {
            return;
        };
        let members: Vec<Statement> = info.members().map(|(stmt, _)| stmt).collect();
        let ports: Vec<SSAValue> = info.ports().iter().map(|&port| port.into()).collect();

        for member in members {
            self.tombstone_statement(member, None);
        }
        for port in ports {
            let _ = self.stage.ssas.delete(port);
        }
        let _ = self.stage.nodes.ungraphs.delete(graph);
    }

    /// The live `UnGraph` `graph`, or [`RewriteError::UnknownUnGraph`] for a
    /// deleted or unknown id.
    pub(in crate::rewrite) fn live_ungraph(
        &self,
        graph: UnGraph,
    ) -> Result<&UnGraphInfo<L>, RewriteError> {
        graph
            .get_info(self.stage)
            .map(|item| &**item)
            .ok_or(RewriteError::UnknownUnGraph(graph))
    }
}
