//! Integration tests for digraph and ungraph builders.

mod common;

use common::{BuilderDialect, TestType, make_wire, new_stage};
use kirin_ir::*;

// --- DiGraph tests ---

#[test]
fn digraph_builder_two_node_dag() {
    let mut stage = new_stage();

    let s0 = stage.statement().definition(BuilderDialect::Nop).new();
    let result_ssa = stage
        .ssa()
        .ty(TestType::I32)
        .kind(BuilderSSAKind::Result(s0, 0))
        .new();

    let s1 = stage
        .statement()
        .definition(BuilderDialect::Add(result_ssa, result_ssa))
        .new();

    let dg = stage.digraph().node(s0).node(s1).name("test_dag").new();

    let stage = stage.into_inner();
    let info = dg.expect_info(&stage);
    assert_eq!(info.graph().node_count(), 2);
    assert_eq!(info.graph().edge_count(), 2); // two operands both ref s0

    assert_eq!(*s0.parent(&stage), Some(StatementParent::DiGraph(dg)));
    assert_eq!(*s1.parent(&stage), Some(StatementParent::DiGraph(dg)));
}

#[test]
fn digraph_builder_port_and_capture_creation() {
    let mut stage = new_stage();

    let dg = stage
        .digraph()
        .port(TestType::I32)
        .port_name("q0")
        .port(TestType::I32)
        .port_name("q1")
        .capture(TestType::I32)
        .capture_name("theta")
        .new();

    let stage = stage.into_inner();
    let info = dg.expect_info(&stage);
    assert_eq!(info.ports().len(), 3);
    assert_eq!(info.edge_count(), 2);
    assert_eq!(info.edge_ports().len(), 2);
    assert_eq!(info.capture_ports().len(), 1);

    let edge0 = info.edge_ports()[0];
    let ssa0 = edge0.expect_info(&stage);
    assert_eq!(*ssa0.kind(), SSAKind::Port(PortParent::DiGraph(dg), 0));

    let edge1 = info.edge_ports()[1];
    let ssa1 = edge1.expect_info(&stage);
    assert_eq!(*ssa1.kind(), SSAKind::Port(PortParent::DiGraph(dg), 1));

    let cap0 = info.capture_ports()[0];
    let ssa_cap = cap0.expect_info(&stage);
    assert_eq!(*ssa_cap.kind(), SSAKind::Port(PortParent::DiGraph(dg), 2));

    assert!(ssa0.name().is_some());
    assert!(ssa1.name().is_some());
    assert!(ssa_cap.name().is_some());
}

#[test]
fn digraph_builder_resolves_builder_port_placeholders() {
    let mut stage = new_stage();

    let placeholder = stage.graph_port().index(0);

    let s0 = stage
        .statement()
        .definition(BuilderDialect::Add(placeholder, placeholder))
        .new();

    let dg = stage
        .digraph()
        .port(TestType::I32)
        .port_name("q0")
        .node(s0)
        .new();

    // Check placeholder deletion via builder arena before converting
    let placeholder_item = stage.ssa_arena().get(placeholder).unwrap();
    assert!(placeholder_item.deleted());

    let stage = stage.into_inner();
    let info = dg.expect_info(&stage);
    let real_port: SSAValue = info.edge_ports()[0].into();

    match s0.definition(&stage) {
        BuilderDialect::Add(a, b) => {
            assert_eq!(*a, real_port);
            assert_eq!(*b, real_port);
        }
        _ => panic!("expected Add"),
    }
}

// --- UnGraph tests ---

#[test]
fn ungraph_two_nodes_one_edge() {
    let mut stage = new_stage();

    let (wire_stmt, wire_ssa) = make_wire(&mut stage);

    let n0 = stage
        .statement()
        .definition(BuilderDialect::Use(wire_ssa))
        .new();
    let n1 = stage
        .statement()
        .definition(BuilderDialect::Use(wire_ssa))
        .new();

    let ug = stage
        .ungraph()
        .edge(wire_stmt)
        .node(n0)
        .node(n1)
        .name("test_ug")
        .new();

    let stage = stage.into_inner();
    let info = ug.expect_info(&stage);
    assert_eq!(info.graph().node_count(), 2);
    assert_eq!(info.graph().edge_count(), 1);
    assert_eq!(info.edge_statements().len(), 1);

    assert_eq!(*n0.parent(&stage), Some(StatementParent::UnGraph(ug)));
    assert_eq!(*n1.parent(&stage), Some(StatementParent::UnGraph(ug)));
    assert_eq!(
        *wire_stmt.parent(&stage),
        Some(StatementParent::UnGraph(ug))
    );
}

#[test]
fn ungraph_boundary_port_bfs_ordering() {
    let mut stage = new_stage();

    let port_placeholder = stage.graph_port().index(0);

    let (wire0_stmt, wire0_ssa) = make_wire(&mut stage);
    let (wire1_stmt, wire1_ssa) = make_wire(&mut stage);

    // n_far uses wire0 only (not connected to boundary)
    // n_mid uses wire0 and wire1 (bridge)
    // n_near uses wire1 and boundary port (connected to boundary)
    let n_far = stage
        .statement()
        .definition(BuilderDialect::Use(wire0_ssa))
        .new();
    let n_mid = stage
        .statement()
        .definition(BuilderDialect::Gate(wire0_ssa, wire1_ssa))
        .new();
    let n_near = stage
        .statement()
        .definition(BuilderDialect::Gate(wire1_ssa, port_placeholder))
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

    let stage = stage.into_inner();
    let info = ug.expect_info(&stage);
    assert_eq!(info.graph().node_count(), 3);

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
    let mut stage = new_stage();

    let (wire_stmt, wire_ssa) = make_wire(&mut stage);

    let n0 = stage
        .statement()
        .definition(BuilderDialect::Use(wire_ssa))
        .new();
    let n1 = stage
        .statement()
        .definition(BuilderDialect::Use(wire_ssa))
        .new();
    let n2 = stage
        .statement()
        .definition(BuilderDialect::Use(wire_ssa))
        .new();

    // This should panic: 3 nodes using the same wire violates 2-use constraint
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
    let mut stage = new_stage();

    let (wire0_stmt, wire0_ssa) = make_wire(&mut stage);
    let (wire1_stmt, wire1_ssa) = make_wire(&mut stage);

    let n0 = stage
        .statement()
        .definition(BuilderDialect::Gate(wire0_ssa, wire1_ssa))
        .new();
    let n1 = stage
        .statement()
        .definition(BuilderDialect::Use(wire0_ssa))
        .new();
    let n2 = stage
        .statement()
        .definition(BuilderDialect::Use(wire1_ssa))
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

    let stage = stage.into_inner();
    let info = ug.expect_info(&stage);
    assert_eq!(info.graph().node_count(), 3);
    assert_eq!(info.graph().edge_count(), 2);
    assert_eq!(info.edge_statements().len(), 2);

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
fn ungraph_port_and_capture_creation() {
    let mut stage = new_stage();

    let (wire_stmt, wire_ssa) = make_wire(&mut stage);

    let n0 = stage
        .statement()
        .definition(BuilderDialect::Use(wire_ssa))
        .new();
    let n1 = stage
        .statement()
        .definition(BuilderDialect::Use(wire_ssa))
        .new();

    let ug = stage
        .ungraph()
        .port(TestType::Qubit)
        .port_name("q0")
        .port(TestType::Qubit)
        .port_name("q1")
        .capture(TestType::I32)
        .capture_name("theta")
        .edge(wire_stmt)
        .node(n0)
        .node(n1)
        .new();

    let stage = stage.into_inner();
    let info = ug.expect_info(&stage);
    // Total ports = 2 edge + 1 capture = 3
    assert_eq!(info.ports().len(), 3);
    assert_eq!(info.edge_count(), 2);
    assert_eq!(info.edge_ports().len(), 2);
    assert_eq!(info.capture_ports().len(), 1);

    // Edge ports get indices 0..N
    let edge0 = info.edge_ports()[0];
    let ssa0 = edge0.expect_info(&stage);
    assert_eq!(*ssa0.kind(), SSAKind::Port(PortParent::UnGraph(ug), 0));

    let edge1 = info.edge_ports()[1];
    let ssa1 = edge1.expect_info(&stage);
    assert_eq!(*ssa1.kind(), SSAKind::Port(PortParent::UnGraph(ug), 1));

    // Capture port gets index N (after edge ports)
    let cap0 = info.capture_ports()[0];
    let ssa_cap = cap0.expect_info(&stage);
    assert_eq!(*ssa_cap.kind(), SSAKind::Port(PortParent::UnGraph(ug), 2));

    // Verify names
    assert!(ssa0.name().is_some());
    assert!(ssa1.name().is_some());
    assert!(ssa_cap.name().is_some());
}

#[test]
fn ungraph_isolated_node_appended_after_bfs() {
    let mut stage = new_stage();

    // Create a boundary port placeholder so BFS has a seed
    let port_placeholder = stage
        .ssa()
        .ty(TestType::Qubit)
        .kind(BuilderSSAKind::Unresolved(ResolutionInfo::Port(
            BuilderKey::Index(0),
        )))
        .new();

    let (wire_stmt, wire_ssa) = make_wire(&mut stage);
    // n0 uses wire + boundary port (BFS seed)
    let n0 = stage
        .statement()
        .definition(BuilderDialect::Gate(wire_ssa, port_placeholder))
        .new();
    let n1 = stage
        .statement()
        .definition(BuilderDialect::Use(wire_ssa))
        .new();

    // Create an isolated node with no edge connections
    let n_isolated = stage.statement().definition(BuilderDialect::Isolated).new();

    // Insert isolated node first — BFS should place it last
    let ug = stage
        .ungraph()
        .port(TestType::Qubit)
        .edge(wire_stmt)
        .node(n_isolated)
        .node(n0)
        .node(n1)
        .new();

    let stage = stage.into_inner();
    let info = ug.expect_info(&stage);
    assert_eq!(info.graph().node_count(), 3);

    let node_order: Vec<Statement> = info
        .graph()
        .node_indices()
        .map(|ni| info.graph()[ni])
        .collect();
    assert_eq!(node_order[2], n_isolated, "isolated node should be last");

    assert_eq!(
        *n_isolated.parent(&stage),
        Some(StatementParent::UnGraph(ug))
    );
}
