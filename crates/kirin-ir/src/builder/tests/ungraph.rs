use crate::arena::GetInfo;
use crate::node::ssa::{SSAInfo, SSAKind, SSAValue};
use crate::node::stmt::StatementParent;
use crate::node::*;
use crate::{
    Block, DiGraph, Dialect, HasArguments, HasArgumentsMut, HasBlocks, HasBlocksMut, HasDigraphs,
    HasDigraphsMut, HasRegions, HasRegionsMut, HasResults, HasResultsMut, HasSuccessors,
    HasSuccessorsMut, HasUngraphs, HasUngraphsMut, IsConstant, IsEdge, IsPure, IsSpeculatable,
    IsTerminator, Region, ResultValue, StageInfo, Successor, UnGraph,
};

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

impl<'a> HasArguments<'a> for UgDialect {
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

impl<'a> HasArgumentsMut<'a> for UgDialect {
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

impl<'a> HasResults<'a> for UgDialect {
    type Iter = std::vec::IntoIter<&'a ResultValue>;
    fn results(&'a self) -> Self::Iter {
        match self {
            UgDialect::Wire(r) => vec![r].into_iter(),
            _ => vec![].into_iter(),
        }
    }
}

impl<'a> HasResultsMut<'a> for UgDialect {
    type IterMut = std::vec::IntoIter<&'a mut ResultValue>;
    fn results_mut(&'a mut self) -> Self::IterMut {
        match self {
            UgDialect::Wire(r) => vec![r].into_iter(),
            _ => vec![].into_iter(),
        }
    }
}

impl<'a> HasBlocks<'a> for UgDialect {
    type Iter = std::iter::Empty<&'a Block>;
    fn blocks(&'a self) -> Self::Iter {
        std::iter::empty()
    }
}

impl<'a> HasBlocksMut<'a> for UgDialect {
    type IterMut = std::iter::Empty<&'a mut Block>;
    fn blocks_mut(&'a mut self) -> Self::IterMut {
        std::iter::empty()
    }
}

impl<'a> HasSuccessors<'a> for UgDialect {
    type Iter = std::iter::Empty<&'a Successor>;
    fn successors(&'a self) -> Self::Iter {
        std::iter::empty()
    }
}

impl<'a> HasSuccessorsMut<'a> for UgDialect {
    type IterMut = std::iter::Empty<&'a mut Successor>;
    fn successors_mut(&'a mut self) -> Self::IterMut {
        std::iter::empty()
    }
}

impl<'a> HasRegions<'a> for UgDialect {
    type Iter = std::iter::Empty<&'a Region>;
    fn regions(&'a self) -> Self::Iter {
        std::iter::empty()
    }
}

impl<'a> HasRegionsMut<'a> for UgDialect {
    type IterMut = std::iter::Empty<&'a mut Region>;
    fn regions_mut(&'a mut self) -> Self::IterMut {
        std::iter::empty()
    }
}

impl IsTerminator for UgDialect {
    fn is_terminator(&self) -> bool {
        false
    }
}

impl IsConstant for UgDialect {
    fn is_constant(&self) -> bool {
        false
    }
}

impl IsPure for UgDialect {
    fn is_pure(&self) -> bool {
        true
    }
}

impl IsSpeculatable for UgDialect {
    fn is_speculatable(&self) -> bool {
        true
    }
}

impl<'a> HasDigraphs<'a> for UgDialect {
    type Iter = std::iter::Empty<&'a DiGraph>;
    fn digraphs(&'a self) -> Self::Iter {
        std::iter::empty()
    }
}

impl<'a> HasDigraphsMut<'a> for UgDialect {
    type IterMut = std::iter::Empty<&'a mut DiGraph>;
    fn digraphs_mut(&'a mut self) -> Self::IterMut {
        std::iter::empty()
    }
}

impl<'a> HasUngraphs<'a> for UgDialect {
    type Iter = std::iter::Empty<&'a UnGraph>;
    fn ungraphs(&'a self) -> Self::Iter {
        std::iter::empty()
    }
}

impl<'a> HasUngraphsMut<'a> for UgDialect {
    type IterMut = std::iter::Empty<&'a mut UnGraph>;
    fn ungraphs_mut(&'a mut self) -> Self::IterMut {
        std::iter::empty()
    }
}

impl IsEdge for UgDialect {
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
    assert_eq!(
        node_order[0], n_near,
        "boundary-connected node should be first"
    );
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

#[test]
fn ungraph_isolated_node_appended_after_bfs() {
    let mut stage: StageInfo<UgDialect> = StageInfo::default();

    // Create a boundary port placeholder so BFS has a seed
    let port_placeholder: SSAValue = {
        let id = stage.ssas.next_id();
        let ssa = SSAInfo::new(id, None, TestType::Qubit, SSAKind::BuilderPort(0));
        stage.ssas.alloc(ssa);
        id
    };

    // Create a wire connecting two nodes
    let (wire_stmt, wire_ssa) = make_wire(&mut stage);
    // n0 uses wire + boundary port (BFS seed)
    let n0 = stage
        .statement()
        .definition(UgDialect::Node2(wire_ssa, port_placeholder))
        .new();
    let n1 = stage
        .statement()
        .definition(UgDialect::Node1(wire_ssa))
        .new();

    // Create an isolated node with no edge connections
    let n_isolated = stage
        .statement()
        .definition(UgDialect::Isolated)
        .new();

    // Insert isolated node first — BFS should place it last
    let ug = stage
        .ungraph()
        .port(TestType::Qubit)
        .edge(wire_stmt)
        .node(n_isolated)
        .node(n0)
        .node(n1)
        .new();

    let info = ug.expect_info(&stage);
    assert_eq!(info.graph().node_count(), 3);

    // BFS visits connected nodes first, isolated node last
    let node_order: Vec<Statement> = info
        .graph()
        .node_indices()
        .map(|ni| info.graph()[ni])
        .collect();
    assert_eq!(node_order[2], n_isolated, "isolated node should be last");

    // Verify parent is set on the isolated node too
    assert_eq!(
        *n_isolated.parent(&stage),
        Some(StatementParent::UnGraph(ug))
    );
}
