use crate::arena::GetInfo;
use crate::node::port::PortParent;
use crate::node::ssa::{SSAInfo, SSAKind, SSAValue};
use crate::node::stmt::StatementParent;
use crate::{
    Block, DiGraph, Dialect, HasArguments, HasArgumentsMut, HasBlocks, HasBlocksMut, HasDigraphs,
    HasDigraphsMut, HasRegions, HasRegionsMut, HasResults, HasResultsMut, HasSuccessors,
    HasSuccessorsMut, HasUngraphs, HasUngraphsMut, IsConstant, IsEdge, IsPure, IsSpeculatable,
    IsTerminator, Region, ResultValue, StageInfo, Successor, UnGraph,
};

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

impl<'a> HasArguments<'a> for GraphDialect {
    type Iter = std::vec::IntoIter<&'a SSAValue>;
    fn arguments(&'a self) -> Self::Iter {
        match self {
            GraphDialect::Add(a, b) => vec![a, b].into_iter(),
            _ => vec![].into_iter(),
        }
    }
}

impl<'a> HasArgumentsMut<'a> for GraphDialect {
    type IterMut = std::vec::IntoIter<&'a mut SSAValue>;
    fn arguments_mut(&'a mut self) -> Self::IterMut {
        match self {
            GraphDialect::Add(a, b) => vec![a, b].into_iter(),
            _ => vec![].into_iter(),
        }
    }
}

impl<'a> HasResults<'a> for GraphDialect {
    type Iter = std::iter::Empty<&'a ResultValue>;
    fn results(&'a self) -> Self::Iter {
        std::iter::empty()
    }
}

impl<'a> HasResultsMut<'a> for GraphDialect {
    type IterMut = std::iter::Empty<&'a mut ResultValue>;
    fn results_mut(&'a mut self) -> Self::IterMut {
        std::iter::empty()
    }
}

impl<'a> HasBlocks<'a> for GraphDialect {
    type Iter = std::iter::Empty<&'a Block>;
    fn blocks(&'a self) -> Self::Iter {
        std::iter::empty()
    }
}

impl<'a> HasBlocksMut<'a> for GraphDialect {
    type IterMut = std::iter::Empty<&'a mut Block>;
    fn blocks_mut(&'a mut self) -> Self::IterMut {
        std::iter::empty()
    }
}

impl<'a> HasSuccessors<'a> for GraphDialect {
    type Iter = std::iter::Empty<&'a Successor>;
    fn successors(&'a self) -> Self::Iter {
        std::iter::empty()
    }
}

impl<'a> HasSuccessorsMut<'a> for GraphDialect {
    type IterMut = std::iter::Empty<&'a mut Successor>;
    fn successors_mut(&'a mut self) -> Self::IterMut {
        std::iter::empty()
    }
}

impl<'a> HasRegions<'a> for GraphDialect {
    type Iter = std::iter::Empty<&'a Region>;
    fn regions(&'a self) -> Self::Iter {
        std::iter::empty()
    }
}

impl<'a> HasRegionsMut<'a> for GraphDialect {
    type IterMut = std::iter::Empty<&'a mut Region>;
    fn regions_mut(&'a mut self) -> Self::IterMut {
        std::iter::empty()
    }
}

impl IsTerminator for GraphDialect {
    fn is_terminator(&self) -> bool {
        false
    }
}

impl IsConstant for GraphDialect {
    fn is_constant(&self) -> bool {
        false
    }
}

impl IsPure for GraphDialect {
    fn is_pure(&self) -> bool {
        true
    }
}

impl IsSpeculatable for GraphDialect {
    fn is_speculatable(&self) -> bool {
        true
    }
}

impl<'a> HasDigraphs<'a> for GraphDialect {
    type Iter = std::iter::Empty<&'a DiGraph>;
    fn digraphs(&'a self) -> Self::Iter {
        std::iter::empty()
    }
}

impl<'a> HasDigraphsMut<'a> for GraphDialect {
    type IterMut = std::iter::Empty<&'a mut DiGraph>;
    fn digraphs_mut(&'a mut self) -> Self::IterMut {
        std::iter::empty()
    }
}

impl<'a> HasUngraphs<'a> for GraphDialect {
    type Iter = std::iter::Empty<&'a UnGraph>;
    fn ungraphs(&'a self) -> Self::Iter {
        std::iter::empty()
    }
}

impl<'a> HasUngraphsMut<'a> for GraphDialect {
    type IterMut = std::iter::Empty<&'a mut UnGraph>;
    fn ungraphs_mut(&'a mut self) -> Self::IterMut {
        std::iter::empty()
    }
}

impl IsEdge for GraphDialect {
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
    assert_eq!(*s0.parent(&stage), Some(StatementParent::DiGraph(dg)));
    assert_eq!(*s1.parent(&stage), Some(StatementParent::DiGraph(dg)));
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
