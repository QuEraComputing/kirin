//! Integration tests for the Rewriter's `UnGraph` surgery: inserting nodes
//! and edge statements while keeping each wire's petgraph edge, and its limit
//! of two incidences, in step with the nodes' operands.

mod common;

use std::collections::HashMap;

use common::{BuilderDialect, TestType, make_split, make_wire, new_stage};
use kirin_ir::*;

/// An unordered endpoint pair, in a canonical order.
fn pair(a: Statement, b: Statement) -> [Statement; 2] {
    if format!("{a:?}") <= format!("{b:?}") {
        [a, b]
    } else {
        [b, a]
    }
}

/// `graph`'s petgraph edges, wire → endpoints.
fn edges(stage: &StageInfo<BuilderDialect>, graph: UnGraph) -> HashMap<SSAValue, [Statement; 2]> {
    let petgraph = graph.expect_info(stage).graph();
    let mut out = HashMap::new();
    for edge in petgraph.raw_edges() {
        let ends = pair(petgraph[edge.source()], petgraph[edge.target()]);
        assert!(
            out.insert(edge.weight, ends).is_none(),
            "wire {} has two edges",
            edge.weight
        );
    }
    out
}

/// The edges `graph` should have, recomputed from its nodes' operands: a
/// wire read by exactly two node slots joins those nodes.
fn expected_edges(
    stage: &StageInfo<BuilderDialect>,
    graph: UnGraph,
) -> HashMap<SSAValue, [Statement; 2]> {
    let info = graph.expect_info(stage);
    let mut wires: Vec<SSAValue> = info
        .edge_statements()
        .iter()
        .flat_map(|stmt| stmt.results(stage).map(|&r| SSAValue::from(r)))
        .collect();
    wires.extend(info.edge_ports().iter().map(|&port| SSAValue::from(port)));

    let mut out = HashMap::new();
    for wire in wires {
        let readers: Vec<Statement> = info
            .graph()
            .node_weights()
            .flat_map(|&node| {
                node.arguments(stage)
                    .filter(move |&&arg| arg == wire)
                    .map(move |_| node)
            })
            .collect();
        if let [a, b] = readers[..] {
            out.insert(wire, pair(a, b));
        }
    }
    out
}

/// The petgraph agrees with the operands.
fn assert_topology(stage: &StageInfo<BuilderDialect>, graph: UnGraph) {
    assert_eq!(edges(stage, graph), expected_edges(stage, graph));
}

fn dead_value() -> SSAValue {
    SSAValue::from(TestSSAValue(9_999))
}

/// An UnGraph with no nodes yet: two wires, a boundary port, and a capture.
struct Fixture {
    stage: StageInfo<BuilderDialect>,
    ug: UnGraph,
    w_stmt: Statement,
    w: SSAValue,
    v_stmt: Statement,
    v: SSAValue,
    boundary: SSAValue,
    capture: SSAValue,
}

fn fixture() -> Fixture {
    let mut stage = new_stage();
    let (w_stmt, w) = make_wire(&mut stage);
    let (v_stmt, v) = make_wire(&mut stage);
    let ug = stage
        .ungraph()
        .port(TestType::Qubit)
        .capture(TestType::I32)
        .edge(w_stmt)
        .edge(v_stmt)
        .new();

    let stage = stage.finalize().unwrap();
    let info = ug.expect_info(&stage);
    let boundary = SSAValue::from(info.edge_ports()[0]);
    let capture = SSAValue::from(info.capture_ports()[0]);
    Fixture {
        stage,
        ug,
        w_stmt,
        w,
        v_stmt,
        v,
        boundary,
        capture,
    }
}

#[test]
fn insert_ungraph_node_connects_the_readers_of_a_wire() {
    let Fixture {
        mut stage,
        ug,
        w,
        boundary,
        capture,
        ..
    } = fixture();
    let mut rw = Rewriter::new(&mut stage);

    // One reader: an incidence, but no edge yet.
    let a = rw.insert_ungraph_node(ug, BuilderDialect::Use(w)).unwrap();
    assert!(edges(rw.stage(), ug).is_empty());
    // A second reader completes the edge; the boundary port gets one reader.
    let b = rw
        .insert_ungraph_node(ug, BuilderDialect::Gate(w, boundary))
        .unwrap();
    assert_eq!(edges(rw.stage(), ug), HashMap::from([(w, pair(a, b))]));
    // A third reader would make `w` a hyperedge.
    assert_eq!(
        rw.insert_ungraph_node(ug, BuilderDialect::Use(w)),
        Err(RewriteError::UnGraphDegreeExceeded {
            graph: ug,
            value: w,
            incidences: 3
        })
    );
    // A boundary port is a wire too; a capture is an ordinary use.
    let d = rw
        .insert_ungraph_node(ug, BuilderDialect::Gate(boundary, capture))
        .unwrap();
    let events = rw.drain_events();
    drop(rw);

    let topology = MutationEvent::ChangedUnGraphTopology { graph: ug };
    assert_eq!(
        events,
        vec![
            MutationEvent::InsertedStatement { stmt: a },
            topology,
            MutationEvent::InsertedStatement { stmt: b },
            topology,
            MutationEvent::InsertedStatement { stmt: d },
            topology,
        ]
    );
    assert_eq!(
        edges(&stage, ug),
        HashMap::from([(w, pair(a, b)), (boundary, pair(b, d))])
    );
    assert_topology(&stage, ug);
    let info = ug.expect_info(&stage);
    for node in [a, b, d] {
        assert_eq!(*node.parent(&stage), Some(StatementParent::UnGraph(ug)));
        assert_eq!(info.graph()[info.node_idx_of(node).unwrap()], node);
    }
    verify_derived(&stage).unwrap();
}

#[test]
fn insert_ungraph_node_reading_a_wire_twice_is_a_self_loop() {
    let Fixture {
        mut stage, ug, v, ..
    } = fixture();
    let mut rw = Rewriter::new(&mut stage);

    let s = rw
        .insert_ungraph_node(ug, BuilderDialect::Gate(v, v))
        .unwrap();
    assert_eq!(edges(rw.stage(), ug), HashMap::from([(v, pair(s, s))]));
    // Both of `v`'s incidences are taken.
    assert_eq!(
        rw.insert_ungraph_node(ug, BuilderDialect::Use(v)),
        Err(RewriteError::UnGraphDegreeExceeded {
            graph: ug,
            value: v,
            incidences: 3
        })
    );
    drop(rw);

    assert_topology(&stage, ug);
    verify_derived(&stage).unwrap();
}

#[test]
fn insert_ungraph_edge_adds_a_wire_nodes_can_read() {
    let Fixture { mut stage, ug, .. } = fixture();
    let mut rw = Rewriter::new(&mut stage);

    let r = rw.new_result(TestType::Qubit);
    let e = rw.insert_ungraph_edge(ug, BuilderDialect::Wire(r)).unwrap();
    // The new wire has no readers, so no edge.
    assert!(edges(rw.stage(), ug).is_empty());
    let n1 = rw
        .insert_ungraph_node(ug, BuilderDialect::Use(r.into()))
        .unwrap();
    let n2 = rw
        .insert_ungraph_node(ug, BuilderDialect::Use(r.into()))
        .unwrap();
    let events = rw.drain_events();
    drop(rw);

    let topology = MutationEvent::ChangedUnGraphTopology { graph: ug };
    assert_eq!(
        events,
        vec![
            MutationEvent::InsertedStatement { stmt: e },
            topology,
            MutationEvent::InsertedStatement { stmt: n1 },
            topology,
            MutationEvent::InsertedStatement { stmt: n2 },
            topology,
        ]
    );
    let info = ug.expect_info(&stage);
    assert!(info.edge_statements().contains(&e));
    assert_eq!(info.node_idx_of(e), None);
    assert_eq!(*r.get_info(&stage).unwrap().kind(), SSAKind::Result(e, 0));
    assert_eq!(
        edges(&stage, ug),
        HashMap::from([(SSAValue::from(r), pair(n1, n2))])
    );
    assert_topology(&stage, ug);
    verify_derived(&stage).unwrap();
}

#[test]
fn ungraph_inserts_reject_without_mutating() {
    let Fixture {
        mut stage, ug, w, ..
    } = fixture();
    let stmts_before = stage.statement_arena().len();
    let dead_graph = UnGraph::from(Id::from(TestSSAValue(9_999)));
    let dead_block = Block::from(Id::from(TestSSAValue(9_999)));

    let mut rw = Rewriter::new(&mut stage);
    let fresh = rw.new_result(TestType::Qubit);
    let rejections = [
        (
            rw.insert_ungraph_node(dead_graph, BuilderDialect::Isolated),
            RewriteError::UnknownUnGraph(dead_graph),
        ),
        (
            rw.insert_ungraph_edge(dead_graph, BuilderDialect::Wire(fresh)),
            RewriteError::UnknownUnGraph(dead_graph),
        ),
        (
            rw.insert_ungraph_node(ug, BuilderDialect::Return),
            RewriteError::UnexpectedTerminator,
        ),
        (
            rw.insert_ungraph_node(ug, BuilderDialect::Fork(Successor::from_block(dead_block))),
            RewriteError::NonTerminatorWithSuccessors,
        ),
        (
            rw.insert_ungraph_node(ug, BuilderDialect::Wire(fresh)),
            RewriteError::UnexpectedEdgeStatement,
        ),
        (
            rw.insert_ungraph_node(ug, BuilderDialect::Use(dead_value())),
            RewriteError::UnknownValue(dead_value()),
        ),
        (
            rw.insert_ungraph_edge(ug, BuilderDialect::Use(w)),
            RewriteError::ExpectedEdgeStatement,
        ),
        (
            rw.insert_ungraph_edge(ug, BuilderDialect::ParamWire(w, fresh)),
            RewriteError::EdgeStatementWithOperands,
        ),
        (
            rw.insert_ungraph_edge(ug, BuilderDialect::Wire(ResultValue::from(w))),
            RewriteError::ResultNotFresh(w),
        ),
    ];
    for (result, expected) in rejections {
        assert_eq!(result, Err(expected));
    }
    assert!(rw.events().is_empty());
    drop(rw);

    assert_eq!(stage.statement_arena().len(), stmts_before);
    let info = ug.expect_info(&stage);
    assert_eq!(info.graph().node_count(), 0);
    assert_eq!(info.edge_statements().len(), 2);
    verify_derived(&stage).unwrap();
}

#[test]
fn erase_ungraph_node_drops_its_edges_and_frees_the_wire() {
    let Fixture {
        mut stage,
        ug,
        w,
        v,
        ..
    } = fixture();
    let mut rw = Rewriter::new(&mut stage);
    let a = rw.insert_ungraph_node(ug, BuilderDialect::Use(w)).unwrap();
    let b = rw.insert_ungraph_node(ug, BuilderDialect::Use(w)).unwrap();
    let c = rw.insert_ungraph_node(ug, BuilderDialect::Use(v)).unwrap();
    rw.drain_events();

    // `a` is not the last node, so the last one moves into its slot.
    rw.erase_statement(a).unwrap();
    assert_eq!(
        rw.drain_events(),
        vec![
            MutationEvent::ErasedStatement { stmt: a },
            MutationEvent::ChangedUnGraphTopology { graph: ug },
        ]
    );
    // `w` is left with one reader, so its edge is gone and it can take another.
    assert!(edges(rw.stage(), ug).is_empty());
    let d = rw.insert_ungraph_node(ug, BuilderDialect::Use(w)).unwrap();
    drop(rw);

    assert!(a.get_info(&stage).is_none());
    assert_eq!(edges(&stage, ug), HashMap::from([(w, pair(b, d))]));
    assert_topology(&stage, ug);
    let info = ug.expect_info(&stage);
    assert_eq!(info.node_idx_of(a), None);
    for node in [b, c, d] {
        assert_eq!(info.graph()[info.node_idx_of(node).unwrap()], node);
    }
    verify_derived(&stage).unwrap();
}

#[test]
fn erase_ungraph_node_with_used_results_is_rejected() {
    let Fixture {
        mut stage,
        ug,
        capture,
        ..
    } = fixture();
    let mut rw = Rewriter::new(&mut stage);
    let r = rw.new_result(TestType::I32);
    let n = rw
        .insert_ungraph_node(ug, BuilderDialect::Compute(capture, r))
        .unwrap();
    let _reader = rw
        .insert_ungraph_node(ug, BuilderDialect::Use(r.into()))
        .unwrap();
    rw.drain_events();

    assert_eq!(
        rw.erase_statement(n),
        Err(RewriteError::StatementResultsInUse(n))
    );
    assert!(rw.events().is_empty());
    drop(rw);

    assert!(n.get_info(&stage).is_some());
    verify_derived(&stage).unwrap();
}

#[test]
fn erase_edge_statement_once_no_node_reads_its_wire() {
    let Fixture {
        mut stage,
        ug,
        w_stmt,
        w,
        v_stmt,
        ..
    } = fixture();
    let mut rw = Rewriter::new(&mut stage);
    let a = rw.insert_ungraph_node(ug, BuilderDialect::Use(w)).unwrap();
    rw.drain_events();

    // A node still reads the wire.
    assert_eq!(
        rw.erase_statement(w_stmt),
        Err(RewriteError::StatementResultsInUse(w_stmt))
    );
    rw.erase_statement(a).unwrap();
    rw.drain_events();
    rw.erase_statement(w_stmt).unwrap();
    assert_eq!(
        rw.drain_events(),
        vec![
            MutationEvent::ErasedStatement { stmt: w_stmt },
            MutationEvent::ChangedUnGraphTopology { graph: ug },
        ]
    );
    drop(rw);

    assert_eq!(ug.expect_info(&stage).edge_statements(), [v_stmt]);
    assert!(w_stmt.get_info(&stage).is_none());
    assert!(w.get_info(&stage).is_none());
    assert_topology(&stage, ug);
    verify_derived(&stage).unwrap();
}

#[test]
fn erase_statement_tombstones_owned_ungraph() {
    let mut stage = new_stage();

    // `owner` holds an ungraph with a boundary port and a wire, whose nodes
    // read the wire and `x` from the outer block.
    let (split, x, _) = make_split(&mut stage);
    let (wire_stmt, wire) = make_wire(&mut stage);
    let n0 = stage
        .statement()
        .definition(BuilderDialect::Gate(wire, x))
        .new();
    let n1 = stage
        .statement()
        .definition(BuilderDialect::Use(wire))
        .new();
    let ug = stage
        .ungraph()
        .port(TestType::Qubit)
        .edge(wire_stmt)
        .node(n0)
        .node(n1)
        .new();
    let dg = stage.digraph().new();
    let owner = stage
        .statement()
        .definition(BuilderDialect::OwnGraphs(dg, ug))
        .new();
    let outer = stage.block().stmt(split).stmt(owner).new();

    let mut stage = stage.finalize().unwrap();
    let port = SSAValue::from(ug.expect_info(&stage).ports()[0]);
    assert!(!x.get_info(&stage).unwrap().uses().is_empty());

    {
        let mut rw = Rewriter::new(&mut stage);
        rw.erase_statement(owner).unwrap();
        assert_eq!(
            rw.drain_events(),
            vec![MutationEvent::ErasedStatement { stmt: owner }]
        );
    }

    // The ungraph and everything in it is gone; the nodes' use of `x` went
    // with them.
    assert!(ug.get_info(&stage).is_none());
    for stmt in [n0, n1, wire_stmt] {
        assert!(stmt.get_info(&stage).is_none());
    }
    assert!(wire.get_info(&stage).is_none());
    assert!(port.get_info(&stage).is_none());
    assert!(x.get_info(&stage).unwrap().uses().is_empty());
    assert_eq!(outer.statements(&stage).collect::<Vec<_>>(), vec![split]);
    verify_derived(&stage).unwrap();
}
