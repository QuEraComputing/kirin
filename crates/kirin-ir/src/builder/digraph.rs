use std::collections::HashMap;

use petgraph::algo::toposort;

use crate::arena::GetInfo;
use crate::node::digraph::{DiGraph, DiGraphInfo};
use crate::node::port::{Port, PortParent};
use crate::node::ssa::{SSAInfo, SSAKind, SSAValue};
use crate::node::stmt::{Statement, StatementParent};
use crate::{Dialect, StageInfo};

pub struct DiGraphBuilder<'a, L: Dialect> {
    stage: &'a mut StageInfo<L>,
    parent: Option<Statement>,
    name: Option<String>,
    ports: Vec<(L::Type, Option<String>)>,
    captures: Vec<(L::Type, Option<String>)>,
    nodes: Vec<Statement>,
    yields: Vec<SSAValue>,
}

impl<'a, L: Dialect> DiGraphBuilder<'a, L> {
    pub(crate) fn from_stage(stage: &'a mut StageInfo<L>) -> Self {
        DiGraphBuilder {
            stage,
            parent: None,
            name: None,
            ports: Vec::new(),
            captures: Vec::new(),
            nodes: Vec::new(),
            yields: Vec::new(),
        }
    }

    /// Attach the digraph to a parent statement.
    pub fn parent(mut self, stmt: Statement) -> Self {
        self.parent = Some(stmt);
        self
    }

    /// Set the name/label of this digraph.
    pub fn name<S: Into<String>>(mut self, name: S) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Add an edge port with a given type.
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

    /// Add a statement as a graph node.
    pub fn node(mut self, stmt: Statement) -> Self {
        self.nodes.push(stmt);
        self
    }

    /// Add a yield output value.
    pub fn yield_value(mut self, ssa: SSAValue) -> Self {
        self.yields.push(ssa);
        self
    }

    /// Finalize the digraph and add it to the stage.
    #[allow(clippy::wrong_self_convention, clippy::new_ret_no_self)]
    pub fn new(self) -> DiGraph {
        let id = self.stage.digraphs.next_id();
        let edge_count = self.ports.len();

        // Step 1: Create Port SSAValues for edge ports (indices 0..N)
        let mut all_ports = Vec::with_capacity(self.ports.len() + self.captures.len());
        for (index, (ty, name)) in self.ports.into_iter().enumerate() {
            let port: Port = self.stage.ssas.next_id().into();
            let ssa = SSAInfo::new(
                port.into(),
                name.map(|n| self.stage.symbols.intern(n)),
                ty,
                SSAKind::Port(PortParent::DiGraph(id), index),
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
                SSAKind::Port(PortParent::DiGraph(id), index),
            );
            self.stage.ssas.alloc(ssa);
            all_ports.push(port);
        }

        // Step 3: Resolve BuilderPort(index) placeholders in node statement operands
        for &stmt_id in &self.nodes {
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

        // Step 4: Build petgraph::DiGraph<Statement, SSAValue>
        // Map Statement -> NodeIndex for nodes in this graph
        let mut stmt_to_node: HashMap<Statement, petgraph::graph::NodeIndex> = HashMap::new();
        let mut graph = petgraph::Graph::<Statement, SSAValue, petgraph::Directed>::new();

        for &stmt_id in &self.nodes {
            let ni = graph.add_node(stmt_id);
            stmt_to_node.insert(stmt_id, ni);
        }

        // For each node's operands, if the operand's producer is also in this graph, add an edge
        for &stmt_id in &self.nodes {
            let consumer_ni = stmt_to_node[&stmt_id];
            let info = stmt_id.expect_info(self.stage);
            let operands: Vec<SSAValue> = info.definition.arguments().copied().collect();
            for operand in operands {
                let ssa_info = self
                    .stage
                    .ssas
                    .get(operand)
                    .expect("SSAValue not found in stage");
                if let SSAKind::Result(producer_stmt, _) = ssa_info.kind
                    && let Some(&producer_ni) = stmt_to_node.get(&producer_stmt)
                {
                    graph.add_edge(producer_ni, consumer_ni, operand);
                }
            }
        }

        // Step 5: Topological sort and reorder
        let mut nodes = self.nodes;
        if let Ok(order) = toposort(&graph, None) {
            let mut new_graph =
                petgraph::Graph::<Statement, SSAValue, petgraph::Directed>::new();
            let mut old_to_new: HashMap<petgraph::graph::NodeIndex, petgraph::graph::NodeIndex> =
                HashMap::new();
            let mut reordered_nodes = Vec::with_capacity(order.len());

            for &old_ni in &order {
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

            graph = new_graph;
            nodes = reordered_nodes;
        }

        // Step 6: Set StatementParent::DiGraph on all node statements
        for &stmt_id in &nodes {
            let info = &mut self.stage.statements[stmt_id];
            info.parent = Some(StatementParent::DiGraph(id));
        }

        // Step 7: Create DiGraphInfo and allocate
        let name_symbol = self.name.map(|n| self.stage.symbols.intern(n));
        let info = DiGraphInfo::new(
            id,
            self.parent,
            name_symbol,
            all_ports,
            edge_count,
            graph,
            self.yields,
        );
        self.stage.digraphs.alloc(info);
        id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arena::GetInfo;
    use crate::node::*;

    // Re-use TestType and RichDialect from context tests
    #[derive(Clone, Debug, PartialEq, Eq, Hash)]
    enum TestType {
        I32,
    }

    impl std::fmt::Display for TestType {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                TestType::I32 => write!(f, "i32"),
            }
        }
    }

    // CompileTimeValue is blanket-implemented for Clone + Debug + Hash + PartialEq

    impl crate::Placeholder for TestType {
        fn placeholder() -> Self {
            TestType::I32
        }
    }

    #[derive(Clone, Debug, PartialEq, Eq, Hash)]
    enum GraphDialect {
        Nop,
        Add(SSAValue, SSAValue),
    }

    impl<'a> crate::HasArguments<'a> for GraphDialect {
        type Iter = std::vec::IntoIter<&'a SSAValue>;
        fn arguments(&'a self) -> Self::Iter {
            match self {
                GraphDialect::Add(a, b) => vec![a, b].into_iter(),
                _ => vec![].into_iter(),
            }
        }
    }

    impl<'a> crate::HasArgumentsMut<'a> for GraphDialect {
        type IterMut = std::vec::IntoIter<&'a mut SSAValue>;
        fn arguments_mut(&'a mut self) -> Self::IterMut {
            match self {
                GraphDialect::Add(a, b) => vec![a, b].into_iter(),
                _ => vec![].into_iter(),
            }
        }
    }

    impl<'a> crate::HasResults<'a> for GraphDialect {
        type Iter = std::iter::Empty<&'a ResultValue>;
        fn results(&'a self) -> Self::Iter {
            std::iter::empty()
        }
    }

    impl<'a> crate::HasResultsMut<'a> for GraphDialect {
        type IterMut = std::iter::Empty<&'a mut ResultValue>;
        fn results_mut(&'a mut self) -> Self::IterMut {
            std::iter::empty()
        }
    }

    impl<'a> crate::HasBlocks<'a> for GraphDialect {
        type Iter = std::iter::Empty<&'a Block>;
        fn blocks(&'a self) -> Self::Iter {
            std::iter::empty()
        }
    }

    impl<'a> crate::HasBlocksMut<'a> for GraphDialect {
        type IterMut = std::iter::Empty<&'a mut Block>;
        fn blocks_mut(&'a mut self) -> Self::IterMut {
            std::iter::empty()
        }
    }

    impl<'a> crate::HasSuccessors<'a> for GraphDialect {
        type Iter = std::iter::Empty<&'a Successor>;
        fn successors(&'a self) -> Self::Iter {
            std::iter::empty()
        }
    }

    impl<'a> crate::HasSuccessorsMut<'a> for GraphDialect {
        type IterMut = std::iter::Empty<&'a mut Successor>;
        fn successors_mut(&'a mut self) -> Self::IterMut {
            std::iter::empty()
        }
    }

    impl<'a> crate::HasRegions<'a> for GraphDialect {
        type Iter = std::iter::Empty<&'a Region>;
        fn regions(&'a self) -> Self::Iter {
            std::iter::empty()
        }
    }

    impl<'a> crate::HasRegionsMut<'a> for GraphDialect {
        type IterMut = std::iter::Empty<&'a mut Region>;
        fn regions_mut(&'a mut self) -> Self::IterMut {
            std::iter::empty()
        }
    }

    impl crate::IsTerminator for GraphDialect {
        fn is_terminator(&self) -> bool {
            false
        }
    }

    impl crate::IsConstant for GraphDialect {
        fn is_constant(&self) -> bool {
            false
        }
    }

    impl crate::IsPure for GraphDialect {
        fn is_pure(&self) -> bool {
            true
        }
    }

    impl crate::IsSpeculatable for GraphDialect {
        fn is_speculatable(&self) -> bool {
            true
        }
    }

    impl<'a> crate::HasDigraphs<'a> for GraphDialect {
        type Iter = std::iter::Empty<&'a DiGraph>;
        fn digraphs(&'a self) -> Self::Iter {
            std::iter::empty()
        }
    }

    impl<'a> crate::HasDigraphsMut<'a> for GraphDialect {
        type IterMut = std::iter::Empty<&'a mut DiGraph>;
        fn digraphs_mut(&'a mut self) -> Self::IterMut {
            std::iter::empty()
        }
    }

    impl<'a> crate::HasUngraphs<'a> for GraphDialect {
        type Iter = std::iter::Empty<&'a UnGraph>;
        fn ungraphs(&'a self) -> Self::Iter {
            std::iter::empty()
        }
    }

    impl<'a> crate::HasUngraphsMut<'a> for GraphDialect {
        type IterMut = std::iter::Empty<&'a mut UnGraph>;
        fn ungraphs_mut(&'a mut self) -> Self::IterMut {
            std::iter::empty()
        }
    }

    impl crate::IsEdge for GraphDialect {
        fn is_edge(&self) -> bool {
            false
        }
    }

    impl Dialect for GraphDialect {
        type Type = TestType;
    }

    #[test]
    fn digraph_builder_two_node_dag() {
        let mut stage: StageInfo<GraphDialect> = StageInfo::default();

        // Create two Nop statements; the second "uses" the first via Result kind
        let s0 = stage.statement().definition(GraphDialect::Nop).new();
        // Manually create an SSA result for s0
        let result_ssa = stage
            .ssa()
            .ty(TestType::I32)
            .kind(SSAKind::Result(s0, 0))
            .new();

        let s1 = stage
            .statement()
            .definition(GraphDialect::Add(result_ssa, result_ssa))
            .new();

        let dg = stage.digraph().node(s0).node(s1).name("test_dag").new();

        let info = dg.expect_info(&stage);
        assert_eq!(info.graph().node_count(), 2);
        assert_eq!(info.graph().edge_count(), 2); // two operands both ref s0

        // Verify parent is set
        assert_eq!(
            *s0.parent(&stage),
            Some(StatementParent::DiGraph(dg))
        );
        assert_eq!(
            *s1.parent(&stage),
            Some(StatementParent::DiGraph(dg))
        );
    }

    #[test]
    fn digraph_builder_port_and_capture_creation() {
        let mut stage: StageInfo<GraphDialect> = StageInfo::default();

        let dg = stage
            .digraph()
            .port(TestType::I32)
            .port_name("q0")
            .port(TestType::I32)
            .port_name("q1")
            .capture(TestType::I32)
            .capture_name("theta")
            .new();

        let info = dg.expect_info(&stage);
        // Total ports = 2 edge + 1 capture = 3
        assert_eq!(info.ports().len(), 3);
        assert_eq!(info.edge_count(), 2);
        assert_eq!(info.edge_ports().len(), 2);
        assert_eq!(info.capture_ports().len(), 1);

        // Verify SSA kinds
        let edge0 = info.edge_ports()[0];
        let ssa0 = edge0.expect_info(&stage);
        assert_eq!(ssa0.kind, SSAKind::Port(PortParent::DiGraph(dg), 0));

        let edge1 = info.edge_ports()[1];
        let ssa1 = edge1.expect_info(&stage);
        assert_eq!(ssa1.kind, SSAKind::Port(PortParent::DiGraph(dg), 1));

        let cap0 = info.capture_ports()[0];
        let ssa_cap = cap0.expect_info(&stage);
        assert_eq!(ssa_cap.kind, SSAKind::Port(PortParent::DiGraph(dg), 2));

        // Verify names
        assert!(ssa0.name().is_some());
        assert!(ssa1.name().is_some());
        assert!(ssa_cap.name().is_some());
    }

    #[test]
    fn digraph_builder_resolves_builder_port_placeholders() {
        let mut stage: StageInfo<GraphDialect> = StageInfo::default();

        // Create a BuilderPort placeholder SSAValue (index 0)
        let placeholder: SSAValue = {
            let id = stage.ssas.next_id();
            let ssa = SSAInfo::new(id, None, TestType::I32, SSAKind::BuilderPort(0));
            stage.ssas.alloc(ssa);
            id
        };

        // Create a statement that uses the placeholder
        let s0 = stage
            .statement()
            .definition(GraphDialect::Add(placeholder, placeholder))
            .new();

        let dg = stage
            .digraph()
            .port(TestType::I32)
            .port_name("q0")
            .node(s0)
            .new();

        let info = dg.expect_info(&stage);
        let real_port: SSAValue = info.edge_ports()[0].into();

        // Verify the statement's operands now point to the real port
        let stmt_info = s0.expect_info(&stage);
        match &stmt_info.definition {
            GraphDialect::Add(a, b) => {
                assert_eq!(*a, real_port);
                assert_eq!(*b, real_port);
            }
            _ => panic!("expected Add"),
        }

        // Verify the placeholder SSA was deleted
        let placeholder_item = stage.ssas.get(placeholder).unwrap();
        assert!(placeholder_item.deleted());
    }
}
