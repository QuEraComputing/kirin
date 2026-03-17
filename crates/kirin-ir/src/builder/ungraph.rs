use std::collections::{HashMap, HashSet, VecDeque};

use crate::arena::GetInfo;
use crate::node::port::{Port, PortParent};
use crate::node::ssa::{SSAInfo, SSAKind, SSAValue};
use crate::node::stmt::{Statement, StatementParent};
use crate::node::ungraph::{UnGraph, UnGraphInfo};
use crate::{Dialect, StageInfo};

pub struct UnGraphBuilder<'a, L: Dialect> {
    stage: &'a mut StageInfo<L>,
    parent: Option<Statement>,
    name: Option<String>,
    ports: Vec<(L::Type, Option<String>)>,
    captures: Vec<(L::Type, Option<String>)>,
    edge_stmts: Vec<Statement>,
    nodes: Vec<Statement>,
}

impl<'a, L: Dialect> UnGraphBuilder<'a, L> {
    pub(crate) fn from_stage(stage: &'a mut StageInfo<L>) -> Self {
        UnGraphBuilder {
            stage,
            parent: None,
            name: None,
            ports: Vec::new(),
            captures: Vec::new(),
            edge_stmts: Vec::new(),
            nodes: Vec::new(),
        }
    }

    /// Attach the ungraph to a parent statement.
    pub fn parent(mut self, stmt: Statement) -> Self {
        self.parent = Some(stmt);
        self
    }

    /// Set the name/label of this ungraph.
    pub fn name<S: Into<String>>(mut self, name: S) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Add a boundary edge port with a given type.
    pub fn port<T: Into<L::Type>>(mut self, ty: T) -> Self {
        self.ports.push((ty.into(), None));
        self
    }

    /// Name the most recently added port.
    ///
    /// Must be called immediately after [`port`](Self::port).
    pub fn port_name<S: Into<String>>(mut self, name: S) -> Self {
        debug_assert!(
            !self.ports.is_empty(),
            "port_name called without a preceding port()"
        );
        if let Some(last) = self.ports.last_mut() {
            last.1 = Some(name.into());
        }
        self
    }

    /// Add a capture port with a given type.
    pub fn capture<T: Into<L::Type>>(mut self, ty: T) -> Self {
        self.captures.push((ty.into(), None));
        self
    }

    /// Name the most recently added capture port.
    ///
    /// Must be called immediately after [`capture`](Self::capture).
    pub fn capture_name<S: Into<String>>(mut self, name: S) -> Self {
        debug_assert!(
            !self.captures.is_empty(),
            "capture_name called without a preceding capture()"
        );
        if let Some(last) = self.captures.last_mut() {
            last.1 = Some(name.into());
        }
        self
    }

    /// Add an edge statement (produces a ResultValue representing a hyperedge/wire).
    pub fn edge(mut self, stmt: Statement) -> Self {
        self.edge_stmts.push(stmt);
        self
    }

    /// Add a node statement.
    pub fn node(mut self, stmt: Statement) -> Self {
        self.nodes.push(stmt);
        self
    }

    /// Finalize the ungraph and add it to the stage.
    #[allow(clippy::wrong_self_convention, clippy::new_ret_no_self)]
    pub fn new(self) -> UnGraph {
        let id = self.stage.ungraphs.next_id();
        let edge_count = self.ports.len();

        // Step 1: Create Port SSAValues for edge ports (indices 0..N)
        let mut all_ports = Vec::with_capacity(self.ports.len() + self.captures.len());
        for (index, (ty, name)) in self.ports.into_iter().enumerate() {
            let port: Port = self.stage.ssas.next_id().into();
            let ssa = SSAInfo::new(
                port.into(),
                name.map(|n| self.stage.symbols.intern(n)),
                ty,
                SSAKind::Port(PortParent::UnGraph(id), index),
            );
            self.stage.ssas.alloc(ssa);
            all_ports.push(port);
        }

        // Step 2: Create Port SSAValues for capture ports (indices N..N+M)
        for (i, (ty, name)) in self.captures.into_iter().enumerate() {
            let index = edge_count + i;
            let port: Port = self.stage.ssas.next_id().into();
            let ssa = SSAInfo::new(
                port.into(),
                name.map(|n| self.stage.symbols.intern(n)),
                ty,
                SSAKind::Port(PortParent::UnGraph(id), index),
            );
            self.stage.ssas.alloc(ssa);
            all_ports.push(port);
        }

        // Step 3: Resolve BuilderPort(index) placeholders in node AND edge statement operands
        let all_stmts: Vec<Statement> = self
            .nodes
            .iter()
            .chain(self.edge_stmts.iter())
            .copied()
            .collect();
        for &stmt_id in &all_stmts {
            let info = &mut self.stage.statements[stmt_id];
            for arg in info.definition.arguments_mut() {
                let ssa_info = self
                    .stage
                    .ssas
                    .get(*arg)
                    .expect("SSAValue not found in stage");
                if let SSAKind::BuilderPort(port_index) = ssa_info.kind {
                    self.stage.ssas.delete(*arg);
                    *arg = all_ports[port_index].into();
                }
            }
        }

        // Step 4: Build petgraph::UnGraph<Statement, SSAValue>
        // First, collect the set of edge SSAValues (ResultValues produced by edge_stmts)
        let mut edge_ssa_set: HashSet<SSAValue> = HashSet::new();
        for &edge_stmt in &self.edge_stmts {
            let info = edge_stmt.expect_info(self.stage);
            for result in info.definition.results() {
                edge_ssa_set.insert((*result).into());
            }
        }
        // Also include boundary port SSAValues as "edge" SSAValues for graph wiring
        let boundary_ssa_set: HashSet<SSAValue> = all_ports
            .iter()
            .take(edge_count)
            .map(|p| (*p).into())
            .collect();

        // Build map: edge SSAValue -> list of node statements that use it
        let mut edge_ssa_to_nodes: HashMap<SSAValue, Vec<Statement>> = HashMap::new();
        for &node_stmt in &self.nodes {
            let info = node_stmt.expect_info(self.stage);
            let operands: Vec<SSAValue> = info.definition.arguments().copied().collect();
            for operand in operands {
                if edge_ssa_set.contains(&operand) || boundary_ssa_set.contains(&operand) {
                    edge_ssa_to_nodes
                        .entry(operand)
                        .or_default()
                        .push(node_stmt);
                }
            }
        }

        // Validate: no edge SSAValue used by more than 2 node statements
        for (ssa, nodes) in &edge_ssa_to_nodes {
            if nodes.len() > 2 {
                panic!(
                    "UnGraph constraint violated: edge SSAValue {} is used by {} node statements \
                     (max 2 allowed for undirected graph edges)",
                    ssa,
                    nodes.len()
                );
            }
        }

        // Build the petgraph
        let mut stmt_to_node: HashMap<Statement, petgraph::graph::NodeIndex> = HashMap::new();
        let mut graph =
            petgraph::Graph::<Statement, SSAValue, petgraph::Undirected>::new_undirected();

        for &stmt_id in &self.nodes {
            let ni = graph.add_node(stmt_id);
            stmt_to_node.insert(stmt_id, ni);
        }

        // For each edge SSAValue used by exactly 2 nodes, add an undirected edge
        for (ssa, nodes) in &edge_ssa_to_nodes {
            if nodes.len() == 2 {
                let n0 = stmt_to_node[&nodes[0]];
                let n1 = stmt_to_node[&nodes[1]];
                graph.add_edge(n0, n1, *ssa);
            }
            // If used by 1 node, it's a dangling/boundary edge — no petgraph edge needed
        }

        // Step 5: BFS reindex from boundary-port-connected nodes
        let mut visited_nodes: HashSet<petgraph::graph::NodeIndex> = HashSet::new();
        let mut visited_edges: HashSet<SSAValue> = HashSet::new();
        let mut bfs_node_order: Vec<petgraph::graph::NodeIndex> = Vec::new();
        let mut bfs_edge_order: Vec<Statement> = Vec::new();
        let mut queue: VecDeque<petgraph::graph::NodeIndex> = VecDeque::new();

        // Build map: edge SSAValue -> edge statement
        let mut ssa_to_edge_stmt: HashMap<SSAValue, Statement> = HashMap::new();
        for &edge_stmt in &self.edge_stmts {
            let info = edge_stmt.expect_info(self.stage);
            for result in info.definition.results() {
                ssa_to_edge_stmt.insert((*result).into(), edge_stmt);
            }
        }

        // Seed BFS with nodes that use boundary port SSAValues
        for &node_stmt in &self.nodes {
            let info = node_stmt.expect_info(self.stage);
            let operands: Vec<SSAValue> = info.definition.arguments().copied().collect();
            let uses_boundary = operands.iter().any(|op| boundary_ssa_set.contains(op));
            if uses_boundary {
                let ni = stmt_to_node[&node_stmt];
                if visited_nodes.insert(ni) {
                    queue.push_back(ni);
                    bfs_node_order.push(ni);
                }
            }
        }

        // BFS traversal
        while let Some(ni) = queue.pop_front() {
            let stmt = graph[ni];
            // Find all edge SSAValues this node uses
            let info = stmt.expect_info(self.stage);
            let operands: Vec<SSAValue> = info.definition.arguments().copied().collect();
            for operand in operands {
                if !visited_edges.contains(&operand) && edge_ssa_set.contains(&operand) {
                    visited_edges.insert(operand);
                    // Record the edge statement in BFS order
                    if let Some(&edge_stmt) = ssa_to_edge_stmt.get(&operand) {
                        bfs_edge_order.push(edge_stmt);
                    }
                    // Find the other endpoint node(s)
                    if let Some(nodes) = edge_ssa_to_nodes.get(&operand) {
                        for &other_stmt in nodes {
                            let other_ni = stmt_to_node[&other_stmt];
                            if visited_nodes.insert(other_ni) {
                                queue.push_back(other_ni);
                                bfs_node_order.push(other_ni);
                            }
                        }
                    }
                }
            }
        }

        // Append any remaining unvisited nodes (isolated)
        for &stmt_id in &self.nodes {
            let ni = stmt_to_node[&stmt_id];
            if visited_nodes.insert(ni) {
                bfs_node_order.push(ni);
            }
        }

        // Append any remaining unvisited edge statements
        let bfs_edge_set: HashSet<Statement> = bfs_edge_order.iter().copied().collect();
        for &edge_stmt in &self.edge_stmts {
            if !bfs_edge_set.contains(&edge_stmt) {
                bfs_edge_order.push(edge_stmt);
            }
        }

        // Rebuild petgraph in BFS node order, remap edges
        let mut new_graph =
            petgraph::Graph::<Statement, SSAValue, petgraph::Undirected>::new_undirected();
        let mut old_to_new: HashMap<petgraph::graph::NodeIndex, petgraph::graph::NodeIndex> =
            HashMap::new();
        let mut reordered_nodes = Vec::with_capacity(bfs_node_order.len());

        for &old_ni in &bfs_node_order {
            let stmt = graph[old_ni];
            let new_ni = new_graph.add_node(stmt);
            old_to_new.insert(old_ni, new_ni);
            reordered_nodes.push(stmt);
        }

        for edge in graph.edge_indices() {
            let (src, dst) = graph.edge_endpoints(edge).unwrap();
            let weight = graph[edge];
            new_graph.add_edge(old_to_new[&src], old_to_new[&dst], weight);
        }

        let graph = new_graph;
        let edge_stmts = bfs_edge_order;

        // Step 6: Set StatementParent::UnGraph on all node + edge statements
        for &stmt_id in &reordered_nodes {
            let info = &mut self.stage.statements[stmt_id];
            info.parent = Some(StatementParent::UnGraph(id));
        }
        for &stmt_id in &edge_stmts {
            let info = &mut self.stage.statements[stmt_id];
            info.parent = Some(StatementParent::UnGraph(id));
        }

        // Step 7: Create UnGraphInfo and allocate
        let name_symbol = self.name.map(|n| self.stage.symbols.intern(n));
        let info = UnGraphInfo::new(
            id,
            self.parent,
            name_symbol,
            all_ports,
            edge_count,
            graph,
            edge_stmts,
        );
        self.stage.ungraphs.alloc(info);
        id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arena::GetInfo;
    use crate::node::*;

    #[derive(Clone, Debug, PartialEq, Eq, Hash)]
    enum TestType {
        Qubit,
    }

    impl std::fmt::Display for TestType {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                TestType::Qubit => write!(f, "qubit"),
            }
        }
    }

    impl crate::Placeholder for TestType {
        fn placeholder() -> Self {
            TestType::Qubit
        }
    }

    /// A node statement: takes edge SSAValues as operands, produces no results.
    #[derive(Clone, Debug, PartialEq, Eq, Hash)]
    enum UgDialect {
        /// A node that references two edge SSAValues
        Node2(SSAValue, SSAValue),
        /// A node that references one edge SSAValue
        Node1(SSAValue),
        /// An edge that produces a ResultValue
        Wire(ResultValue),
        /// A node with no operands (isolated)
        Isolated,
    }

    impl<'a> crate::HasArguments<'a> for UgDialect {
        type Iter = std::vec::IntoIter<&'a SSAValue>;
        fn arguments(&'a self) -> Self::Iter {
            match self {
                UgDialect::Node2(a, b) => vec![a, b].into_iter(),
                UgDialect::Node1(a) => vec![a].into_iter(),
                UgDialect::Wire(_) => vec![].into_iter(),
                UgDialect::Isolated => vec![].into_iter(),
            }
        }
    }

    impl<'a> crate::HasArgumentsMut<'a> for UgDialect {
        type IterMut = std::vec::IntoIter<&'a mut SSAValue>;
        fn arguments_mut(&'a mut self) -> Self::IterMut {
            match self {
                UgDialect::Node2(a, b) => vec![a, b].into_iter(),
                UgDialect::Node1(a) => vec![a].into_iter(),
                UgDialect::Wire(_) => vec![].into_iter(),
                UgDialect::Isolated => vec![].into_iter(),
            }
        }
    }

    impl<'a> crate::HasResults<'a> for UgDialect {
        type Iter = std::vec::IntoIter<&'a ResultValue>;
        fn results(&'a self) -> Self::Iter {
            match self {
                UgDialect::Wire(r) => vec![r].into_iter(),
                _ => vec![].into_iter(),
            }
        }
    }

    impl<'a> crate::HasResultsMut<'a> for UgDialect {
        type IterMut = std::vec::IntoIter<&'a mut ResultValue>;
        fn results_mut(&'a mut self) -> Self::IterMut {
            match self {
                UgDialect::Wire(r) => vec![r].into_iter(),
                _ => vec![].into_iter(),
            }
        }
    }

    impl<'a> crate::HasBlocks<'a> for UgDialect {
        type Iter = std::iter::Empty<&'a Block>;
        fn blocks(&'a self) -> Self::Iter {
            std::iter::empty()
        }
    }

    impl<'a> crate::HasBlocksMut<'a> for UgDialect {
        type IterMut = std::iter::Empty<&'a mut Block>;
        fn blocks_mut(&'a mut self) -> Self::IterMut {
            std::iter::empty()
        }
    }

    impl<'a> crate::HasSuccessors<'a> for UgDialect {
        type Iter = std::iter::Empty<&'a Successor>;
        fn successors(&'a self) -> Self::Iter {
            std::iter::empty()
        }
    }

    impl<'a> crate::HasSuccessorsMut<'a> for UgDialect {
        type IterMut = std::iter::Empty<&'a mut Successor>;
        fn successors_mut(&'a mut self) -> Self::IterMut {
            std::iter::empty()
        }
    }

    impl<'a> crate::HasRegions<'a> for UgDialect {
        type Iter = std::iter::Empty<&'a Region>;
        fn regions(&'a self) -> Self::Iter {
            std::iter::empty()
        }
    }

    impl<'a> crate::HasRegionsMut<'a> for UgDialect {
        type IterMut = std::iter::Empty<&'a mut Region>;
        fn regions_mut(&'a mut self) -> Self::IterMut {
            std::iter::empty()
        }
    }

    impl crate::IsTerminator for UgDialect {
        fn is_terminator(&self) -> bool {
            false
        }
    }

    impl crate::IsConstant for UgDialect {
        fn is_constant(&self) -> bool {
            false
        }
    }

    impl crate::IsPure for UgDialect {
        fn is_pure(&self) -> bool {
            true
        }
    }

    impl crate::IsSpeculatable for UgDialect {
        fn is_speculatable(&self) -> bool {
            true
        }
    }

    impl<'a> crate::HasDigraphs<'a> for UgDialect {
        type Iter = std::iter::Empty<&'a DiGraph>;
        fn digraphs(&'a self) -> Self::Iter {
            std::iter::empty()
        }
    }

    impl<'a> crate::HasDigraphsMut<'a> for UgDialect {
        type IterMut = std::iter::Empty<&'a mut DiGraph>;
        fn digraphs_mut(&'a mut self) -> Self::IterMut {
            std::iter::empty()
        }
    }

    impl<'a> crate::HasUngraphs<'a> for UgDialect {
        type Iter = std::iter::Empty<&'a UnGraph>;
        fn ungraphs(&'a self) -> Self::Iter {
            std::iter::empty()
        }
    }

    impl<'a> crate::HasUngraphsMut<'a> for UgDialect {
        type IterMut = std::iter::Empty<&'a mut UnGraph>;
        fn ungraphs_mut(&'a mut self) -> Self::IterMut {
            std::iter::empty()
        }
    }

    impl crate::IsEdge for UgDialect {
        fn is_edge(&self) -> bool {
            matches!(self, UgDialect::Wire(_))
        }
    }

    impl Dialect for UgDialect {
        type Type = TestType;
    }

    /// Helper: create a Wire edge statement that produces a ResultValue.
    fn make_wire(stage: &mut StageInfo<UgDialect>) -> (Statement, SSAValue) {
        // Create a placeholder ResultValue first
        let result_id: ResultValue = stage.ssas.next_id().into();
        let stmt = stage
            .statement()
            .definition(UgDialect::Wire(result_id))
            .new();
        // Now create the SSA result pointing to this statement
        let ssa = SSAInfo::new(
            result_id.into(),
            None,
            TestType::Qubit,
            SSAKind::Result(stmt, 0),
        );
        stage.ssas.alloc(ssa);
        (stmt, result_id.into())
    }

    #[test]
    fn ungraph_two_nodes_one_edge() {
        let mut stage: StageInfo<UgDialect> = StageInfo::default();

        // Create one wire (edge statement)
        let (wire_stmt, wire_ssa) = make_wire(&mut stage);

        // Create two node statements that both use the wire
        let n0 = stage
            .statement()
            .definition(UgDialect::Node1(wire_ssa))
            .new();
        let n1 = stage
            .statement()
            .definition(UgDialect::Node1(wire_ssa))
            .new();

        let ug = stage
            .ungraph()
            .edge(wire_stmt)
            .node(n0)
            .node(n1)
            .name("test_ug")
            .new();

        let info = ug.expect_info(&stage);
        assert_eq!(info.graph().node_count(), 2);
        assert_eq!(info.graph().edge_count(), 1);
        assert_eq!(info.edge_statements().len(), 1);

        // Verify parent is set on nodes and edges
        assert_eq!(*n0.parent(&stage), Some(StatementParent::UnGraph(ug)));
        assert_eq!(*n1.parent(&stage), Some(StatementParent::UnGraph(ug)));
        assert_eq!(
            *wire_stmt.parent(&stage),
            Some(StatementParent::UnGraph(ug))
        );
    }

    #[test]
    fn ungraph_boundary_port_bfs_ordering() {
        let mut stage: StageInfo<UgDialect> = StageInfo::default();

        // Create a boundary port placeholder
        let port_placeholder: SSAValue = {
            let id = stage.ssas.next_id();
            let ssa = SSAInfo::new(id, None, TestType::Qubit, SSAKind::BuilderPort(0));
            stage.ssas.alloc(ssa);
            id
        };

        // Create two wires
        let (wire0_stmt, wire0_ssa) = make_wire(&mut stage);
        let (wire1_stmt, wire1_ssa) = make_wire(&mut stage);

        // n_far uses wire0 only (not connected to boundary)
        // n_mid uses wire0 and wire1 (bridge)
        // n_near uses wire1 and boundary port (connected to boundary)
        let n_far = stage
            .statement()
            .definition(UgDialect::Node1(wire0_ssa))
            .new();
        let n_mid = stage
            .statement()
            .definition(UgDialect::Node2(wire0_ssa, wire1_ssa))
            .new();
        let n_near = stage
            .statement()
            .definition(UgDialect::Node2(wire1_ssa, port_placeholder))
            .new();

        // Insert nodes in reverse BFS order: far first, near last
        let ug = stage
            .ungraph()
            .port(TestType::Qubit)
            .port_name("p0")
            .edge(wire0_stmt)
            .edge(wire1_stmt)
            .node(n_far)
            .node(n_mid)
            .node(n_near)
            .new();

        let info = ug.expect_info(&stage);
        assert_eq!(info.graph().node_count(), 3);

        // BFS starts from boundary-connected nodes.
        // n_near uses boundary port -> visited first
        // n_mid uses wire1 (shared with n_near) -> visited second
        // n_far uses wire0 (shared with n_mid) -> visited third
        let node_order: Vec<Statement> = info
            .graph()
            .node_indices()
            .map(|ni| info.graph()[ni])
            .collect();
        assert_eq!(node_order[0], n_near, "boundary-connected node should be first");
        assert_eq!(node_order[1], n_mid, "bridge node should be second");
        assert_eq!(node_order[2], n_far, "far node should be third");
    }

    #[test]
    #[should_panic(expected = "UnGraph constraint violated")]
    fn ungraph_edge_max_two_uses_validation() {
        let mut stage: StageInfo<UgDialect> = StageInfo::default();

        // Create one wire
        let (wire_stmt, wire_ssa) = make_wire(&mut stage);

        // Create three nodes all using the same wire — violates the 2-use constraint
        let n0 = stage
            .statement()
            .definition(UgDialect::Node1(wire_ssa))
            .new();
        let n1 = stage
            .statement()
            .definition(UgDialect::Node1(wire_ssa))
            .new();
        let n2 = stage
            .statement()
            .definition(UgDialect::Node1(wire_ssa))
            .new();

        // This should panic
        stage
            .ungraph()
            .edge(wire_stmt)
            .node(n0)
            .node(n1)
            .node(n2)
            .new();
    }

    #[test]
    fn ungraph_interleaved_edge_node_order() {
        let mut stage: StageInfo<UgDialect> = StageInfo::default();

        let (wire0_stmt, wire0_ssa) = make_wire(&mut stage);
        let (wire1_stmt, wire1_ssa) = make_wire(&mut stage);

        let n0 = stage
            .statement()
            .definition(UgDialect::Node2(wire0_ssa, wire1_ssa))
            .new();
        let n1 = stage
            .statement()
            .definition(UgDialect::Node1(wire0_ssa))
            .new();
        let n2 = stage
            .statement()
            .definition(UgDialect::Node1(wire1_ssa))
            .new();

        // Interleave edges and nodes in insertion order
        let ug = stage
            .ungraph()
            .edge(wire1_stmt)
            .node(n2)
            .edge(wire0_stmt)
            .node(n0)
            .node(n1)
            .new();

        let info = ug.expect_info(&stage);
        assert_eq!(info.graph().node_count(), 3);
        assert_eq!(info.graph().edge_count(), 2);
        assert_eq!(info.edge_statements().len(), 2);

        // All nodes and edges should have correct parent regardless of insertion order
        assert_eq!(*n0.parent(&stage), Some(StatementParent::UnGraph(ug)));
        assert_eq!(*n1.parent(&stage), Some(StatementParent::UnGraph(ug)));
        assert_eq!(*n2.parent(&stage), Some(StatementParent::UnGraph(ug)));
        assert_eq!(
            *wire0_stmt.parent(&stage),
            Some(StatementParent::UnGraph(ug))
        );
        assert_eq!(
            *wire1_stmt.parent(&stage),
            Some(StatementParent::UnGraph(ug))
        );
    }
}
