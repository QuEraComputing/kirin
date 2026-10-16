//! Node and edge edits on a [`GraphInfo`] that keep the petgraph and its
//! statement → node map in lockstep. Crate-private: the rewriter decides which
//! edits to make, these only keep the two stores consistent.

use petgraph::EdgeType;
use petgraph::graph::NodeIndex;
use petgraph::visit::EdgeRef;

use super::graph::GraphInfo;
use crate::{Dialect, SSAValue, Statement};

impl<L: Dialect, D: EdgeType, Extra> GraphInfo<L, D, Extra> {
    /// Add `stmt` as a new node. Returns `false`, changing nothing, if `stmt`
    /// is already a member.
    pub(crate) fn add_node(&mut self, stmt: Statement) -> bool {
        if self.stmt_to_node_idx.contains_key(&stmt) {
            return false;
        }
        let node = self.graph.add_node(stmt);
        self.stmt_to_node_idx.insert(stmt, node);
        true
    }

    /// Remove `stmt`'s node together with its incident edges. Returns `false`
    /// if `stmt` is not a member.
    pub(crate) fn remove_node(&mut self, stmt: Statement) -> bool {
        let Some(node) = self.stmt_to_node_idx.remove(&stmt) else {
            return false;
        };
        self.graph.remove_node(node);
        // `Graph::remove_node` swap-removes: the last node now sits at `node`.
        if let Some(&moved) = self.graph.node_weight(node) {
            self.stmt_to_node_idx.insert(moved, node);
        }
        true
    }

    /// Add an edge `producer → consumer` carrying `value`.
    ///
    /// # Panics
    ///
    /// If either statement is not a member.
    pub(crate) fn add_edge(&mut self, producer: Statement, consumer: Statement, value: SSAValue) {
        let (from, to) = (
            self.expect_node_idx_of(producer),
            self.expect_node_idx_of(consumer),
        );
        self.graph.add_edge(from, to, value);
    }

    /// Remove one edge `producer → consumer` carrying `value` (parallel edges
    /// stand for separate operand slots). Returns `false` if there is none.
    ///
    /// # Panics
    ///
    /// If either statement is not a member.
    pub(crate) fn remove_edge(
        &mut self,
        producer: Statement,
        consumer: Statement,
        value: SSAValue,
    ) -> bool {
        let (from, to) = (
            self.expect_node_idx_of(producer),
            self.expect_node_idx_of(consumer),
        );
        let edge = self
            .graph
            .edges_connecting(from, to)
            .find(|edge| *edge.weight() == value)
            .map(|edge| edge.id());
        edge.is_some_and(|edge| self.graph.remove_edge(edge).is_some())
    }

    fn expect_node_idx_of(&self, stmt: Statement) -> NodeIndex {
        self.node_idx_of(stmt)
            .unwrap_or_else(|| panic!("{stmt:?} is not a graph member"))
    }
}

#[cfg(test)]
mod tests {
    use crate::arena::Id;
    use crate::stage::tests::LangA;
    use crate::{DiGraphExtra, DiGraphInfo, GraphInfo, SSAValue, Statement};

    fn stmt(raw: usize) -> Statement {
        Statement(Id(raw))
    }

    fn value(raw: usize) -> SSAValue {
        SSAValue(Id(raw))
    }

    fn empty_digraph() -> DiGraphInfo<LangA> {
        GraphInfo::new(
            Id(0),
            None,
            None,
            Vec::new(),
            0,
            petgraph::Graph::new(),
            DiGraphExtra::new(Vec::new()),
        )
    }

    /// Every map entry points at its own node, and every node has an entry.
    fn assert_map_in_sync(info: &DiGraphInfo<LangA>) {
        assert_eq!(info.stmt_to_node_idx.len(), info.graph().node_count());
        for node in info.graph().node_indices() {
            assert_eq!(info.node_idx_of(info.graph()[node]), Some(node));
        }
    }

    #[test]
    fn add_node_rejects_existing_member() {
        let mut info = empty_digraph();
        assert!(info.add_node(stmt(0)));
        assert!(!info.add_node(stmt(0)));
        assert_eq!(info.graph().node_count(), 1);
        assert_map_in_sync(&info);
    }

    #[test]
    fn remove_node_repoints_the_swapped_in_node() {
        let mut info = empty_digraph();
        for raw in 0..3 {
            info.add_node(stmt(raw));
        }

        // Removing the first node moves the last one into its slot.
        assert!(info.remove_node(stmt(0)));
        assert_eq!(info.node_idx_of(stmt(0)), None);
        assert_map_in_sync(&info);

        // Removing the last node moves nothing.
        assert!(info.remove_node(stmt(1)));
        assert_map_in_sync(&info);

        assert!(!info.remove_node(stmt(0)));
    }

    #[test]
    fn remove_node_drops_incident_edges() {
        let mut info = empty_digraph();
        for raw in 0..3 {
            info.add_node(stmt(raw));
        }
        info.add_edge(stmt(0), stmt(1), value(10));
        info.add_edge(stmt(1), stmt(2), value(11));

        assert!(info.remove_node(stmt(1)));
        assert_eq!(info.graph().edge_count(), 0);
        assert_map_in_sync(&info);
    }

    #[test]
    fn remove_edge_removes_one_parallel_edge() {
        let mut info = empty_digraph();
        info.add_node(stmt(0));
        info.add_node(stmt(1));
        // Two operand slots of stmt(1) read the same value, one reads another.
        info.add_edge(stmt(0), stmt(1), value(10));
        info.add_edge(stmt(0), stmt(1), value(10));
        info.add_edge(stmt(0), stmt(1), value(11));

        assert!(info.remove_edge(stmt(0), stmt(1), value(10)));
        let mut weights: Vec<_> = info
            .graph()
            .edge_references()
            .map(|e| *e.weight())
            .collect();
        weights.sort_by_key(|v| v.0.raw());
        assert_eq!(weights, [value(10), value(11)]);

        assert!(info.remove_edge(stmt(0), stmt(1), value(10)));
        assert!(!info.remove_edge(stmt(0), stmt(1), value(10)));
        assert!(!info.remove_edge(stmt(1), stmt(0), value(11)));
        assert_eq!(info.graph().edge_count(), 1);
    }
}
