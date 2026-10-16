use crate::arena::{GetInfo, GetInfoMut};
use crate::{Dialect, SSAValue, Statement, StatementParent, UnGraph, UnGraphInfo};

use crate::rewrite::rewriter::{MutationEvent, RewriteError, Rewriter};
use crate::rewrite::ungraph::topology::Incidences;

/// Adding `UnGraph` members.
impl<'a, L: Dialect> Rewriter<'a, L> {
    /// Add `definition` to `graph` as a new node.
    ///
    /// `definition` must not be an edge statement (see
    /// [`Rewriter::insert_ungraph_edge`]), and must be a non-terminator with no
    /// successors. Every operand
    /// must be live, and every result a fresh [`Rewriter::new_result`] value.
    /// Each operand that is a wire of `graph` (an edge statement's result or
    /// a boundary port) becomes an incidence of that wire; a wire read by two
    /// node slots is an edge between those nodes. Other operands are ordinary
    /// uses.
    ///
    /// Rejects, leaving the stage untouched, if `graph` is unknown, a check
    /// above fails, or a wire would end up with more than two incidences
    /// ([`RewriteError::UnGraphDegreeExceeded`]). Records
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

        /// A wire the new node reads.
        struct WireRead {
            value: SSAValue,
            /// How many of the node's operand slots read it.
            reads: usize,
            /// Its incidences before the insert.
            orig_incidences: Incidences,
        }
        let mut wires: Vec<WireRead> = Vec::new();
        for &operand in &operands {
            if let Some(wire) = wires.iter_mut().find(|wire| wire.value == operand) {
                // Wire already seen: the node reads it in another slot.
                wire.reads += 1;
            } else if self.is_edge_value(graph, operand) {
                // A wire read for the first time. Operands that are not wires
                // of this graph are ordinary uses and skipped.
                wires.push(WireRead {
                    value: operand,
                    reads: 1,
                    orig_incidences: self.incidences(graph, operand),
                });
            }
        }
        for wire in &wires {
            let incidences = wire.orig_incidences.len() + wire.reads;
            if incidences > 2 {
                return Err(RewriteError::UnGraphDegreeExceeded {
                    graph,
                    value: wire.value,
                    incidences,
                });
            }
        }

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
        for wire in wires {
            let after = self.incidences(graph, wire.value);
            self.resync_edge(graph, wire.value, &wire.orig_incidences, &after);
        }

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
            .extra
            .edge_statements
            .push(stmt);

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

        let info = graph
            .get_info_mut(self.stage)
            .expect("graph is live in this stage");
        if info.remove_node(stmt) {
            // A node takes its edges with it; each wire it read is left with
            // one incidence fewer.
        } else {
            // An edge statement: no node reads its wires (checked above), so
            // they have no edges; just drop it from the list.
            let edges = &mut info.extra.edge_statements;
            if let Some(pos) = edges.iter().position(|&edge| edge == stmt) {
                edges.swap_remove(pos);
            }
        }
        self.tombstone_statement(stmt, None);

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
        let members: Vec<Statement> = info
            .graph()
            .node_weights()
            .chain(info.edge_statements())
            .copied()
            .collect();
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
