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

/// The wires `verify_derived` reports as read by more than two node slots.
fn degree_findings(stage: &StageInfo<BuilderDialect>) -> Vec<Finding> {
    match verify_derived(stage) {
        Ok(()) => Vec::new(),
        Err(VerifyError::Derive(error)) => error.findings().to_vec(),
        Err(other) => panic!("unexpected verification error: {other}"),
    }
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
    // A third reader makes `w` a hyperedge: accepted, but `w` loses its edge
    // and verification reports it.
    let c = rw.insert_ungraph_node(ug, BuilderDialect::Use(w)).unwrap();
    assert!(edges(rw.stage(), ug).is_empty());
    assert_eq!(
        degree_findings(rw.stage()),
        [Finding::UnGraphDegreeExceeded {
            graph: ug,
            value: w,
            incidences: 3
        }]
    );
    // Back to two readers, the edge returns.
    rw.erase_statement(c).unwrap();
    assert_eq!(edges(rw.stage(), ug), HashMap::from([(w, pair(a, b))]));
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
            MutationEvent::InsertedStatement { stmt: c },
            topology,
            MutationEvent::ErasedStatement { stmt: c },
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
    // Both of `v`'s incidences are taken: a third reader leaves no edge.
    rw.insert_ungraph_node(ug, BuilderDialect::Use(v)).unwrap();
    drop(rw);

    assert!(edges(&stage, ug).is_empty());
    assert_topology(&stage, ug);
    assert_eq!(
        degree_findings(&stage),
        [Finding::UnGraphDegreeExceeded {
            graph: ug,
            value: v,
            incidences: 3
        }]
    );
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
    let owner = stage
        .statement()
        .definition(BuilderDialect::OwnUnGraph(ug))
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

#[test]
fn replace_operand_swaps_endpoints_through_an_over_full_wire() {
    let Fixture {
        mut stage,
        ug,
        w,
        v,
        ..
    } = fixture();
    let mut rw = Rewriter::new(&mut stage);
    // w joins a — b, v joins c — d; both wires are full.
    let a = rw.insert_ungraph_node(ug, BuilderDialect::Use(w)).unwrap();
    let b = rw.insert_ungraph_node(ug, BuilderDialect::Use(w)).unwrap();
    let c = rw.insert_ungraph_node(ug, BuilderDialect::Use(v)).unwrap();
    let d = rw.insert_ungraph_node(ug, BuilderDialect::Use(v)).unwrap();
    rw.drain_events();
    let topology = MutationEvent::ChangedUnGraphTopology { graph: ug };

    // Moving b onto v first leaves v with three readers: no edge, and invalid
    // until the swap completes.
    assert_eq!(rw.replace_operand(b, 0, v), Ok(w));
    assert!(edges(rw.stage(), ug).is_empty());
    assert_topology(rw.stage(), ug);
    assert_eq!(
        degree_findings(rw.stage()),
        [Finding::UnGraphDegreeExceeded {
            graph: ug,
            value: v,
            incidences: 3
        }]
    );
    assert_eq!(
        rw.drain_events(),
        vec![MutationEvent::ChangedOperands { stmt: b }, topology]
    );

    // Moving d onto w completes it: w joins a — d, v joins c — b.
    rw.replace_operand(d, 0, w).unwrap();
    assert_eq!(
        rw.drain_events(),
        vec![MutationEvent::ChangedOperands { stmt: d }, topology]
    );
    drop(rw);

    assert_eq!(
        edges(&stage, ug),
        HashMap::from([(w, pair(a, d)), (v, pair(b, c))])
    );
    assert_topology(&stage, ug);
    verify_derived(&stage).unwrap();
}

#[test]
fn replace_operand_off_a_wire_drops_its_edge() {
    let Fixture {
        mut stage,
        ug,
        w,
        capture,
        ..
    } = fixture();
    let mut rw = Rewriter::new(&mut stage);
    let _a = rw.insert_ungraph_node(ug, BuilderDialect::Use(w)).unwrap();
    let b = rw.insert_ungraph_node(ug, BuilderDialect::Use(w)).unwrap();
    rw.drain_events();

    // A capture is not a wire: b leaves w, so w's edge goes.
    rw.replace_operand(b, 0, capture).unwrap();
    assert!(edges(rw.stage(), ug).is_empty());
    assert_eq!(
        rw.drain_events(),
        vec![
            MutationEvent::ChangedOperands { stmt: b },
            MutationEvent::ChangedUnGraphTopology { graph: ug },
        ]
    );

    // Neither the capture nor a node's result is a wire: moving between them
    // changes no topology, so only the operand event is recorded.
    let r = rw.new_result(TestType::I32);
    rw.insert_ungraph_node(ug, BuilderDialect::Compute(capture, r))
        .unwrap();
    rw.drain_events();
    rw.replace_operand(b, 0, r).unwrap();
    assert_eq!(
        rw.drain_events(),
        vec![MutationEvent::ChangedOperands { stmt: b }]
    );
    drop(rw);

    assert_topology(&stage, ug);
    verify_derived(&stage).unwrap();
}

#[test]
fn replace_all_uses_moves_every_reader_to_the_new_wire() {
    let Fixture {
        mut stage,
        ug,
        w,
        v,
        ..
    } = fixture();
    let mut rw = Rewriter::new(&mut stage);
    let a = rw.insert_ungraph_node(ug, BuilderDialect::Use(w)).unwrap();
    let c = rw.insert_ungraph_node(ug, BuilderDialect::Use(v)).unwrap();
    rw.drain_events();

    // One reader each: merging w into v gives v exactly two.
    assert_eq!(rw.replace_all_uses(w, v), Ok(1));
    assert_eq!(edges(rw.stage(), ug), HashMap::from([(v, pair(a, c))]));
    assert_eq!(
        rw.drain_events(),
        vec![
            MutationEvent::ChangedOperands { stmt: a },
            MutationEvent::ChangedUnGraphTopology { graph: ug },
            MutationEvent::ReplacedUses { old: w, new: v },
        ]
    );

    // A third reader merged in leaves v over full, with no edge.
    let e = rw.insert_ungraph_node(ug, BuilderDialect::Use(w)).unwrap();
    rw.replace_all_uses(w, v).unwrap();
    drop(rw);

    assert!(edges(&stage, ug).is_empty());
    assert_topology(&stage, ug);
    assert_eq!(*e.definition(&stage), BuilderDialect::Use(v));
    assert_eq!(
        degree_findings(&stage),
        [Finding::UnGraphDegreeExceeded {
            graph: ug,
            value: v,
            incidences: 3
        }]
    );
}

#[test]
fn replace_results_redirects_an_edge_statements_readers() {
    let Fixture {
        mut stage,
        ug,
        w_stmt,
        w,
        v,
        ..
    } = fixture();
    let mut rw = Rewriter::new(&mut stage);
    let a = rw.insert_ungraph_node(ug, BuilderDialect::Use(w)).unwrap();
    let b = rw.insert_ungraph_node(ug, BuilderDialect::Use(w)).unwrap();
    rw.drain_events();

    // w's readers move to v, so the edge now carries v, and w_stmt is free
    // to go.
    rw.replace_results(w_stmt, &[v]).unwrap();
    assert_eq!(edges(rw.stage(), ug), HashMap::from([(v, pair(a, b))]));
    rw.erase_statement(w_stmt).unwrap();
    drop(rw);

    assert_topology(&stage, ug);
    verify_derived(&stage).unwrap();
}

#[test]
fn replace_all_uses_across_two_ungraphs() {
    let mut stage = new_stage();

    // `x` from the outer block is read by a node in each of two ungraphs;
    // `w1` is a wire of the first, already read by `a`.
    let (split, x, _) = make_split(&mut stage);
    let _outer = stage.block().stmt(split).new();
    let (w1_stmt, w1) = make_wire(&mut stage);
    let a = stage.statement().definition(BuilderDialect::Use(w1)).new();
    let b = stage.statement().definition(BuilderDialect::Use(x)).new();
    let ug1 = stage.ungraph().edge(w1_stmt).node(a).node(b).new();
    let c = stage.statement().definition(BuilderDialect::Use(x)).new();
    let ug2 = stage.ungraph().node(c).new();

    let mut stage = stage.finalize().unwrap();
    let mut rw = Rewriter::new(&mut stage);
    assert_eq!(rw.replace_all_uses(x, w1), Ok(2));
    let events = rw.drain_events();
    drop(rw);

    // In ug1, `b` joins `w1`; in ug2, `w1` is not a wire, so nothing changes.
    assert_eq!(edges(&stage, ug1), HashMap::from([(w1, pair(a, b))]));
    assert!(edges(&stage, ug2).is_empty());
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, MutationEvent::ChangedUnGraphTopology { .. }))
            .collect::<Vec<_>>(),
        [&MutationEvent::ChangedUnGraphTopology { graph: ug1 }]
    );
    assert_topology(&stage, ug1);
    assert_topology(&stage, ug2);
    verify_derived(&stage).unwrap();
}
