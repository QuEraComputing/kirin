use smallvec::SmallVec;

use crate::arena::{GetInfo, GetInfoMut};
use crate::{DiGraph, DiGraphInfo, Dialect, SSAValue, Statement, StatementParent, Use};

use crate::rewrite::rewriter::{MutationEvent, RewriteError, Rewriter, add_use, remove_use};

/// Adding, erasing, and tombstoning `DiGraph` members, and editing yields.
impl<'a, L: Dialect> Rewriter<'a, L> {
    /// Add `definition` to `graph` as a new member statement and node.
    ///
    /// `definition` must be a non-terminator (a `DiGraph` has none). Every
    /// operand must be live, and every result a fresh
    /// [`Rewriter::new_result`] value, which becomes a result of the new
    /// statement. Each operand slot reading a result of another member gets an
    /// edge from that member; operands from outside the graph get none.
    ///
    /// Rejects, leaving the stage untouched, if `graph` is unknown, or
    /// `definition` is a terminator or fails the operand/result checks.
    /// Records [`MutationEvent::InsertedStatement`] then
    /// [`MutationEvent::ChangedDiGraphTopology`].
    pub fn insert_node(
        &mut self,
        graph: DiGraph,
        definition: L,
    ) -> Result<Statement, RewriteError> {
        self.live_digraph(graph)?;
        if definition.is_terminator() {
            return Err(RewriteError::UnexpectedTerminator);
        }
        let (results, operands) = self.prepare_insert(&definition)?;

        let stmt = self.alloc_statement(
            definition,
            StatementParent::DiGraph(graph),
            None,
            None,
            results,
            &operands,
        );
        graph
            .get_info_mut(self.stage)
            .expect("graph is live in this stage")
            .add_node(stmt);
        for &operand in &operands {
            self.add_operand_edge(graph, stmt, operand);
        }

        self.events.push(MutationEvent::InsertedStatement { stmt });
        self.events
            .push(MutationEvent::ChangedDiGraphTopology { graph });
        Ok(stmt)
    }

    /// The `DiGraph` arm of [`Rewriter::erase_statement`]: `stmt` is a live
    /// member of `graph`.
    pub(in crate::rewrite) fn erase_digraph_member(
        &mut self,
        stmt: Statement,
        graph: DiGraph,
    ) -> Result<(), RewriteError> {
        let results: SmallVec<[SSAValue; 2]> = stmt
            .expect_info(self.stage)
            .definition
            .results()
            .map(|&result| result.into())
            .collect();
        for result in results {
            if result
                .get_info(self.stage)
                .is_some_and(|info| !info.uses().is_empty())
            {
                return Err(RewriteError::StatementResultsInUse(stmt));
            }
        }

        // Its results are unused, so it has no outgoing edges; removing the
        // node drops exactly the edges from its operands.
        if let Some(info) = graph.get_info_mut(self.stage) {
            info.remove_node(stmt);
        }
        self.tombstone_statement(stmt, None);

        self.events.push(MutationEvent::ErasedStatement { stmt });
        self.events
            .push(MutationEvent::ChangedDiGraphTopology { graph });
        Ok(())
    }

    /// Tombstone `graph` with everything in it: its members (recursing into
    /// their nested bodies) with their uses and results, the uses held by its
    /// yields, and its ports. The graph's own topology is left as is.
    pub(in crate::rewrite) fn tombstone_digraph(&mut self, graph: DiGraph) {
        let Ok(info) = self.live_digraph(graph) else {
            return;
        };
        let members: Vec<Statement> = info.graph().node_weights().copied().collect();
        let yields: Vec<SSAValue> = info.yields().to_vec();
        let ports: Vec<SSAValue> = info.ports().iter().map(|&port| port.into()).collect();

        for (index, value) in yields.into_iter().enumerate() {
            remove_use(self.stage, value, Use::DiGraphYield { graph, index });
        }
        for member in members {
            self.tombstone_statement(member, None);
        }
        for port in ports {
            let _ = self.stage.ssas.delete(port);
        }
        let _ = self.stage.nodes.digraphs.delete(graph);
    }

    /// Rewrite the `index`-th yield of `graph` to `value`, returning the
    /// previous value.
    ///
    /// Maintains the def-use index: the yield use moves from the previous
    /// value's [`SSAInfo::uses`](crate::SSAInfo) to `value`'s. Yields are not
    /// petgraph edges, so the topology is unchanged. Writing the value already
    /// in the slot is a no-op and records no event.
    ///
    /// Rejects, leaving the stage untouched, if `value` or `graph` is unknown
    /// or `index` is out of range. Records
    /// [`MutationEvent::ChangedDiGraphYields`].
    pub fn replace_yield(
        &mut self,
        graph: DiGraph,
        index: usize,
        value: impl Into<SSAValue>,
    ) -> Result<SSAValue, RewriteError> {
        let value: SSAValue = value.into();
        if value.get_info(self.stage).is_none() {
            return Err(RewriteError::UnknownValue(value));
        }
        let old = *self
            .live_digraph(graph)?
            .yields()
            .get(index)
            .ok_or(RewriteError::YieldIndexOutOfRange { graph, index })?;
        if old == value {
            return Ok(old);
        }

        graph
            .get_info_mut(self.stage)
            .expect("graph is live in this stage")
            .yields_mut()[index] = value;
        let site = Use::DiGraphYield { graph, index };
        remove_use(self.stage, old, site);
        add_use(self.stage, value, site);

        self.events
            .push(MutationEvent::ChangedDiGraphYields { graph });
        Ok(old)
    }

    /// The live `DiGraph` `graph`, or [`RewriteError::UnknownDiGraph`] for a
    /// deleted or unknown id.
    pub(in crate::rewrite) fn live_digraph(
        &self,
        graph: DiGraph,
    ) -> Result<&DiGraphInfo<L>, RewriteError> {
        graph
            .get_info(self.stage)
            .map(|item| &**item)
            .ok_or(RewriteError::UnknownDiGraph(graph))
    }
}
