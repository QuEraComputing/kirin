use crate::arena::{GetInfo, GetInfoMut};
use crate::{Dialect, SSAValue, Statement, StatementParent, UnGraph, UnGraphInfo};

use crate::rewrite::rewriter::{MutationEvent, RewriteError, Rewriter};

/// Adding `UnGraph` members.
impl<'a, L: Dialect> Rewriter<'a, L> {
    /// Add `definition` to `graph` as a new node.
    ///
    /// `definition` must not be an edge statement (see
    /// [`Rewriter::insert_ungraph_edge`]), and must be a non-terminator with no
    /// successors. Every operand must be live, and every result a fresh
    /// [`Rewriter::new_result`] value. Each operand that is a wire of `graph`
    /// (an edge statement's result or a boundary port) becomes an incidence of
    /// that wire; a wire read by two node slots is an edge between those
    /// nodes. Other operands are ordinary uses.
    ///
    /// A wire may end up with more than two incidences. That is invalid IR,
    /// left for [`verify_derived`](crate::verify_derived) to report; such a
    /// wire has no edge.
    ///
    /// Rejects, leaving the stage untouched, if `graph` is unknown or a check
    /// above fails. Records
    /// [`MutationEvent::InsertedStatement`] then
    /// [`MutationEvent::ChangedUnGraphTopology`].
    pub fn insert_ungraph_node(
        &mut self,
        graph: UnGraph,
        definition: L,
    ) -> Result<Statement, RewriteError> {
        self.live_ungraph(graph)?;
        if definition.is_edge() {
            return Err(RewriteError::UnexpectedEdgeStatement);
        }
        if definition.is_terminator() {
            return Err(RewriteError::UnexpectedTerminator);
        }
        if definition.successors().next().is_some() {
            return Err(RewriteError::NonTerminatorWithSuccessors);
        }
        let (results, operands) = self.prepare_insert(&definition)?;

        let wires = self.snapshot_wires(graph, operands.iter().copied());

        let stmt = self.alloc_statement(
            definition,
            StatementParent::UnGraph(graph),
            None,
            None,
            results,
            &operands,
        );
        graph
            .get_info_mut(self.stage)
            .expect("graph is live in this stage")
            .add_node(stmt);
        self.resync_wires(graph, wires);

        self.events.push(MutationEvent::InsertedStatement { stmt });
        self.events
            .push(MutationEvent::ChangedUnGraphTopology { graph });
        Ok(stmt)
    }

    /// Add `definition` to `graph` as a new edge statement, whose results are
    /// new wires.
    ///
    /// `definition` must be an edge statement, not a terminator, with no
    /// successors and no operands (a wire only connects the nodes that read
    /// it); every result must be a fresh [`Rewriter::new_result`]
    /// value. The new wires have no incidences yet, so the petgraph gains no
    /// edge until nodes read them.
    ///
    /// Rejects, leaving the stage untouched, if `graph` is unknown or a check
    /// above fails. Records [`MutationEvent::InsertedStatement`] then
    /// [`MutationEvent::ChangedUnGraphTopology`].
    pub fn insert_ungraph_edge(
        &mut self,
        graph: UnGraph,
        definition: L,
    ) -> Result<Statement, RewriteError> {
        self.live_ungraph(graph)?;
        if !definition.is_edge() {
            return Err(RewriteError::ExpectedEdgeStatement);
        }
        if definition.is_terminator() {
            return Err(RewriteError::UnexpectedTerminator);
        }
        if definition.successors().next().is_some() {
            return Err(RewriteError::NonTerminatorWithSuccessors);
        }
        if definition.arguments().next().is_some() {
            return Err(RewriteError::EdgeStatementWithOperands);
        }
        let (results, operands) = self.prepare_insert(&definition)?;

        let stmt = self.alloc_statement(
            definition,
            StatementParent::UnGraph(graph),
            None,
            None,
            results,
            &operands,
        );
        graph
            .get_info_mut(self.stage)
            .expect("graph is live in this stage")
            .add_edge_member(stmt);

        self.events.push(MutationEvent::InsertedStatement { stmt });
        self.events
            .push(MutationEvent::ChangedUnGraphTopology { graph });
        Ok(stmt)
    }

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
            // A node: each wire it read loses its incidences. The edge changes
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
