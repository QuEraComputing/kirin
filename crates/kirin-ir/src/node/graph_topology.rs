//! Member and edge edits on a [`GraphInfo`] that keep the petgraph and its
//! member map in lockstep. Crate-private: the rewriter and the builders decide
//! which edits to make, these only keep the two stores consistent.

use petgraph::EdgeType;
use petgraph::graph::NodeIndex;
use petgraph::visit::EdgeRef;

use super::graph::{GraphInfo, GraphMember, MemberRole};
use crate::{DiGraphInfo, Dialect, SSAValue, Statement, UnGraphInfo};

impl<L: Dialect, D: EdgeType, Extra> GraphInfo<L, D, Extra> {
    /// Replace the whole mirror: `members` in the given order (each `Node`
    /// gets a petgraph node), then one petgraph edge per `(a, b, value)`
    /// (from `a` to `b` if directed). Finalization only.
    ///
    /// # Panics
    ///
    /// If an edge endpoint is not a node member.
    pub(crate) fn install_mirror(
        &mut self,
        members: &[(Statement, MemberRole)],
        edges: &[(Statement, Statement, SSAValue)],
    ) {
        self.graph.clear();
        self.members.clear();
        for &(stmt, role) in members {
            let member = match role {
                MemberRole::Node => GraphMember::Node(self.graph.add_node(stmt)),
                MemberRole::Edge => GraphMember::Edge,
            };
            self.members.insert(stmt, member);
        }
        for &(a, b, value) in edges {
            self.insert_edge(a, b, value);
        }
    }

    /// Append `stmt` as a new node. Returns `false`, changing nothing, if
    /// `stmt` is already a member.
    pub(crate) fn add_node(&mut self, stmt: Statement) -> bool {
        if self.members.contains_key(&stmt) {
            return false;
        }
        let node = self.graph.add_node(stmt);
        self.members.insert(stmt, GraphMember::Node(node));
        true
    }

    /// Remove node `stmt` together with its incident edges, in O(1) plus its
    /// degree. The last member takes `stmt`'s place in the member map, so
    /// member order is not kept. Returns `false` if `stmt` is not a node.
    pub(crate) fn remove_node(&mut self, stmt: Statement) -> bool {
        let Some(GraphMember::Node(node)) = self.member(stmt) else {
            return false;
        };
        self.members.swap_remove(&stmt);
        // A `StableGraph` leaves the other nodes where they are.
        self.graph.remove_node(node);
        true
    }

    /// Add an edge between members `a` and `b` (from `a` to `b` if directed).
    fn insert_edge(&mut self, a: Statement, b: Statement, value: SSAValue) {
        let (a, b) = (self.expect_node_idx_of(a), self.expect_node_idx_of(b));
        self.graph.add_edge(a, b, value);
    }

    /// Remove an edge carrying `value` between members `a` and `b` (from `a`
    /// to `b` if directed), returning whether there was one. Of several such
    /// parallel edges, only one is removed.
    fn delete_edge(&mut self, a: Statement, b: Statement, value: SSAValue) -> bool {
        let (a, b) = (self.expect_node_idx_of(a), self.expect_node_idx_of(b));
        let edge = self
            .graph
            .edges_connecting(a, b)
            .find(|edge| *edge.weight() == value)
            .map(|edge| edge.id());
        edge.is_some_and(|edge| self.graph.remove_edge(edge).is_some())
    }

    fn expect_node_idx_of(&self, stmt: Statement) -> NodeIndex {
        self.node_idx_of(stmt)
            .unwrap_or_else(|| panic!("{stmt:?} is not a graph member"))
    }
}

impl<L: Dialect> DiGraphInfo<L> {
    /// Add an edge `producer → consumer` carrying `value`.
    ///
    /// # Panics
    ///
    /// If either statement is not a member.
    pub(crate) fn add_edge(&mut self, producer: Statement, consumer: Statement, value: SSAValue) {
        self.insert_edge(producer, consumer, value);
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
        self.delete_edge(producer, consumer, value)
    }
}

impl<L: Dialect> UnGraphInfo<L> {
    /// Append `stmt` as a new edge statement. Returns `false`, changing
    /// nothing, if `stmt` is already a member.
    pub(crate) fn add_edge_member(&mut self, stmt: Statement) -> bool {
        if self.members.contains_key(&stmt) {
            return false;
        }
        self.members.insert(stmt, GraphMember::Edge);
        true
    }

    /// Remove edge statement `stmt` in O(1). The last member takes its place
    /// in the member map, so member order is not kept. Returns `false` if
    /// `stmt` is not an edge statement.
    pub(crate) fn remove_edge_member(&mut self, stmt: Statement) -> bool {
        if self.member(stmt) != Some(GraphMember::Edge) {
            return false;
        }
        self.members.swap_remove(&stmt);
        true
    }

    /// Add the edge for `wire` between nodes `a` and `b`.
    ///
    /// # Panics
    ///
    /// If either statement is not a node.
    pub(crate) fn add_edge(&mut self, a: Statement, b: Statement, wire: SSAValue) {
        self.insert_edge(a, b, wire);
    }

    /// Remove the edge for `wire` between nodes `a` and `b`, in either order.
    /// Returns `false` if there is none.
    ///
    /// # Panics
    ///
    /// If either statement is not a node.
    pub(crate) fn remove_edge(&mut self, a: Statement, b: Statement, wire: SSAValue) -> bool {
        self.delete_edge(a, b, wire)
    }
}

#[cfg(test)]
mod tests {
    use petgraph::visit::IntoEdgeReferences;

    use crate::arena::Id;
    use crate::stage::tests::LangA;
    use crate::{
        DiGraphExtra, DiGraphInfo, GraphInfo, GraphMember, SSAValue, Statement, UnGraphExtra,
        UnGraphInfo,
    };

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
            DiGraphExtra::new(Vec::new()),
        )
    }

    /// Every map entry points at its own node, and every node has an entry.
    fn assert_map_in_sync(info: &DiGraphInfo<LangA>) {
        assert_eq!(info.members.len(), info.graph().node_count());
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
    fn remove_node_leaves_other_node_indices_unchanged() {
        let mut info = empty_digraph();
        for raw in 0..3 {
            info.add_node(stmt(raw));
        }
        let last = info.node_idx_of(stmt(2));

        assert!(info.remove_node(stmt(0)));
        assert_eq!(info.node_idx_of(stmt(0)), None);
        // A StableGraph does not move the last node into the hole.
        assert_eq!(info.node_idx_of(stmt(2)), last);
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

    fn empty_ungraph() -> UnGraphInfo<LangA> {
        GraphInfo::new(Id(0), None, None, Vec::new(), 0, UnGraphExtra::default())
    }

    #[test]
    fn edge_members_have_no_node_and_removal_swaps_in_the_last_member() {
        let mut info = empty_ungraph();
        info.add_node(stmt(0));
        assert!(info.add_edge_member(stmt(1)));
        assert!(!info.add_edge_member(stmt(1)));
        assert!(!info.add_edge_member(stmt(0)), "already a node");
        info.add_node(stmt(2));

        assert_eq!(info.member(stmt(1)), Some(GraphMember::Edge));
        assert_eq!(info.node_idx_of(stmt(1)), None);
        assert_eq!(info.graph().node_count(), 2);
        assert_eq!(info.edge_statements().collect::<Vec<_>>(), [stmt(1)]);
        assert_eq!(info.nodes().collect::<Vec<_>>(), [stmt(0), stmt(2)]);

        // A node is not an edge member and vice versa. Removing a member is
        // O(1): the last member takes its place in the map.
        assert!(!info.remove_edge_member(stmt(0)));
        assert!(!info.remove_node(stmt(1)));
        assert!(info.remove_node(stmt(0)));
        let members: Vec<_> = info.members().map(|(stmt, _)| stmt).collect();
        assert_eq!(members, [stmt(2), stmt(1)]);
        assert_eq!(info.graph()[info.node_idx_of(stmt(2)).unwrap()], stmt(2));
        assert!(info.remove_edge_member(stmt(1)));
        assert_eq!(info.members().count(), 1);
    }
}
