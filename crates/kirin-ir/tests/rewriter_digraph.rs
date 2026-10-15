//! Integration tests for the Rewriter's `DiGraph` surgery: inserting and
//! erasing member statements with their petgraph nodes and edges, yield
//! edits, and tombstoning a `DiGraph` with its owner.

mod common;

use std::collections::{HashMap, HashSet};

use common::{BuilderDialect, TestType, make_compute, make_split, new_stage};
use kirin_ir::*;

/// The def-use set of `value` (order-agnostic; `SSAInfo::uses` is a bag).
fn uses_set(stage: &StageInfo<BuilderDialect>, value: impl Into<SSAValue>) -> HashSet<Use> {
    let value: SSAValue = value.into();
    value
        .get_info(stage)
        .map(|info| info.uses().iter().copied().collect())
        .unwrap_or_default()
}

fn so(stmt: Statement, index: usize) -> Use {
    Use::StatementOperand { stmt, index }
}

fn dgy(graph: DiGraph, index: usize) -> Use {
    Use::DiGraphYield { graph, index }
}

type Edge = (Statement, Statement, SSAValue);

/// `graph`'s edges as a multiset of `(producer, consumer, value)`.
fn edges(stage: &StageInfo<BuilderDialect>, graph: DiGraph) -> HashMap<Edge, usize> {
    let petgraph = graph.expect_info(stage).graph();
    let mut out = HashMap::new();
    for edge in petgraph.raw_edges() {
        let key = (
            petgraph[edge.source()],
            petgraph[edge.target()],
            edge.weight,
        );
        *out.entry(key).or_insert(0) += 1;
    }
    out
}

/// The members of `graph` are exactly `members`, and every member's node
/// lookup lands on its own node.
fn assert_members(stage: &StageInfo<BuilderDialect>, graph: DiGraph, members: &[Statement]) {
    let info = graph.expect_info(stage);
    let actual: HashSet<Statement> = info.graph().node_weights().copied().collect();
    assert_eq!(actual, members.iter().copied().collect());
    assert_eq!(info.graph().node_count(), members.len());
    for &member in members {
        let node = info.node_idx_of(member).expect("member has a node");
        assert_eq!(info.graph()[node], member);
    }
}

fn dead_value() -> SSAValue {
    SSAValue::from(TestSSAValue(9_999))
}

#[test]
fn insert_node_adds_operand_edges_and_binds_results() {
    let mut stage = new_stage();

    // `x` is defined outside the graph; `p` is a member reading it.
    let (split, x, _) = make_split(&mut stage);
    let _outer = stage.block().stmt(split).new();
    let (p, rp) = make_compute(&mut stage, x);
    let dg = stage.digraph().node(p).new();

    let mut stage = stage.finalize().unwrap();
    assert!(edges(&stage, dg).is_empty());

    let mut rw = Rewriter::new(&mut stage);
    // A member operand gets an edge from its producer.
    let r1 = rw.new_result(TestType::I32);
    let n1 = rw.insert_node(dg, BuilderDialect::Compute(rp, r1)).unwrap();
    // An outside operand gets no edge.
    let n2 = rw
        .insert_node(dg, BuilderDialect::Add(r1.into(), x))
        .unwrap();
    // Two slots reading the same member result get two parallel edges.
    let n3 = rw.insert_node(dg, BuilderDialect::Add(rp, rp)).unwrap();
    let events = rw.drain_events();
    drop(rw);

    let topology = MutationEvent::ChangedDiGraphTopology { graph: dg };
    assert_eq!(
        events,
        vec![
            MutationEvent::InsertedStatement { stmt: n1 },
            topology,
            MutationEvent::InsertedStatement { stmt: n2 },
            topology,
            MutationEvent::InsertedStatement { stmt: n3 },
            topology,
        ]
    );

    assert_members(&stage, dg, &[p, n1, n2, n3]);
    assert_eq!(
        edges(&stage, dg),
        HashMap::from([((p, n1, rp), 1), ((n1, n2, r1.into()), 1), ((p, n3, rp), 2)])
    );
    assert_eq!(*n1.parent(&stage), Some(StatementParent::DiGraph(dg)));
    let info = r1.get_info(&stage).unwrap();
    assert_eq!(*info.kind(), SSAKind::Result(n1, 0));

    assert_eq!(
        uses_set(&stage, rp),
        HashSet::from([so(n1, 0), so(n3, 0), so(n3, 1)])
    );
    assert_eq!(uses_set(&stage, r1), HashSet::from([so(n2, 0)]));
    assert_eq!(uses_set(&stage, x), HashSet::from([so(p, 0), so(n2, 1)]));
    verify_derived(&stage).unwrap();
}

#[test]
fn insert_node_rejects_without_mutating() {
    let mut stage = new_stage();

    let (split, x, _) = make_split(&mut stage);
    let _outer = stage.block().stmt(split).new();
    let (p, rp) = make_compute(&mut stage, x);
    let dg = stage.digraph().node(p).new();

    let mut stage = stage.finalize().unwrap();
    let stmts_before = stage.statement_arena().len();
    let dead_graph = DiGraph::from(Id::from(TestSSAValue(9_999)));

    let mut rw = Rewriter::new(&mut stage);
    let fresh = rw.new_result(TestType::I32);
    assert_eq!(
        rw.insert_node(dead_graph, BuilderDialect::Nop),
        Err(RewriteError::UnknownDiGraph(dead_graph))
    );
    assert_eq!(
        rw.insert_node(dg, BuilderDialect::Return),
        Err(RewriteError::UnexpectedTerminator)
    );
    assert_eq!(
        rw.insert_node(dg, BuilderDialect::Compute(x, ResultValue::from(rp))),
        Err(RewriteError::ResultNotFresh(rp))
    );
    assert_eq!(
        rw.insert_node(dg, BuilderDialect::Use(dead_value())),
        Err(RewriteError::UnknownValue(dead_value()))
    );
    // An unbound result is not live, so it cannot be an operand.
    assert_eq!(
        rw.insert_node(dg, BuilderDialect::Use(fresh.into())),
        Err(RewriteError::UnknownValue(fresh.into()))
    );
    assert!(rw.events().is_empty());
    drop(rw);

    assert_eq!(stage.statement_arena().len(), stmts_before);
    assert_members(&stage, dg, &[p]);
    verify_derived(&stage).unwrap();
}

#[test]
fn erase_statement_removes_digraph_member_and_its_edges() {
    let mut stage = new_stage();

    let (split, x, _) = make_split(&mut stage);
    let _outer = stage.block().stmt(split).new();
    // p → q → u, p → w; y is used only by the graph's yield.
    let (p, rp) = make_compute(&mut stage, x);
    let (q, rq) = make_compute(&mut stage, rp);
    let u = stage.statement().definition(BuilderDialect::Use(rq)).new();
    let w = stage.statement().definition(BuilderDialect::Use(rp)).new();
    let (y, ry) = make_compute(&mut stage, x);
    let dg = stage
        .digraph()
        .node(p)
        .node(q)
        .node(u)
        .node(w)
        .node(y)
        .yield_value(ry)
        .new();

    let mut stage = stage.finalize().unwrap();
    let mut rw = Rewriter::new(&mut stage);

    // Results still read by another member, or by a yield, block erasure.
    assert_eq!(
        rw.erase_statement(q),
        Err(RewriteError::StatementResultsInUse(q))
    );
    assert_eq!(
        rw.erase_statement(y),
        Err(RewriteError::StatementResultsInUse(y))
    );
    assert!(rw.events().is_empty());

    // `u` is not the last node, so the last one moves into its slot.
    rw.erase_statement(u).unwrap();
    // With `u` gone, `q`'s result is unused and `q` can go too.
    rw.erase_statement(q).unwrap();
    let events = rw.drain_events();
    drop(rw);

    let topology = MutationEvent::ChangedDiGraphTopology { graph: dg };
    assert_eq!(
        events,
        vec![
            MutationEvent::ErasedStatement { stmt: u },
            topology,
            MutationEvent::ErasedStatement { stmt: q },
            topology,
        ]
    );

    assert_members(&stage, dg, &[p, w, y]);
    assert_eq!(dg.expect_info(&stage).node_idx_of(u), None);
    assert_eq!(edges(&stage, dg), HashMap::from([((p, w, rp), 1)]));
    assert!(u.get_info(&stage).is_none());
    assert!(q.get_info(&stage).is_none());
    assert!(rq.get_info(&stage).is_none());
    assert_eq!(uses_set(&stage, rp), HashSet::from([so(w, 0)]));
    verify_derived(&stage).unwrap();
}

#[test]
fn replace_yield_moves_the_yield_use() {
    let mut stage = new_stage();

    let (split, x, _) = make_split(&mut stage);
    let _outer = stage.block().stmt(split).new();
    let (p, rp) = make_compute(&mut stage, x);
    let dg = stage.digraph().node(p).yield_value(rp).new();

    let mut stage = stage.finalize().unwrap();
    let dead_graph = DiGraph::from(Id::from(TestSSAValue(9_999)));
    let mut rw = Rewriter::new(&mut stage);

    assert_eq!(
        rw.replace_yield(dg, 1, x),
        Err(RewriteError::YieldIndexOutOfRange {
            graph: dg,
            index: 1
        })
    );
    assert_eq!(
        rw.replace_yield(dead_graph, 0, x),
        Err(RewriteError::UnknownDiGraph(dead_graph))
    );
    assert_eq!(
        rw.replace_yield(dg, 0, dead_value()),
        Err(RewriteError::UnknownValue(dead_value()))
    );
    assert!(rw.events().is_empty());

    assert_eq!(rw.replace_yield(dg, 0, x), Ok(rp));
    // Writing the value already there records nothing.
    assert_eq!(rw.replace_yield(dg, 0, x), Ok(x));
    assert_eq!(
        rw.drain_events(),
        vec![MutationEvent::ChangedDiGraphYields { graph: dg }]
    );
    // No longer yielded, `p` can be erased.
    rw.erase_statement(p).unwrap();
    drop(rw);

    assert_eq!(dg.expect_info(&stage).yields(), [x]);
    assert_eq!(uses_set(&stage, x), HashSet::from([dgy(dg, 0)]));
    assert_members(&stage, dg, &[]);
    verify_derived(&stage).unwrap();
}

#[test]
fn erase_statement_tombstones_owned_digraph() {
    let mut stage = new_stage();

    // `owner` holds a digraph with a port, whose member reads `x` from the
    // outer block and is yielded.
    let (split, x, _) = make_split(&mut stage);
    let (p, rp) = make_compute(&mut stage, x);
    let dg = stage
        .digraph()
        .port(TestType::I32)
        .node(p)
        .yield_value(rp)
        .new();
    let iso = stage.statement().definition(BuilderDialect::Isolated).new();
    let ug = stage.ungraph().node(iso).new();
    let owner = stage
        .statement()
        .definition(BuilderDialect::OwnGraphs(dg, ug))
        .new();
    let outer = stage.block().stmt(split).stmt(owner).new();

    let mut stage = stage.finalize().unwrap();
    let port = SSAValue::from(dg.expect_info(&stage).ports()[0]);
    assert_eq!(uses_set(&stage, x), HashSet::from([so(p, 0)]));

    {
        let mut rw = Rewriter::new(&mut stage);
        rw.erase_statement(owner).unwrap();
        assert_eq!(
            rw.drain_events(),
            vec![MutationEvent::ErasedStatement { stmt: owner }]
        );
    }

    // The digraph and everything in it is gone; the member's use of `x` went
    // with it.
    assert!(dg.get_info(&stage).is_none());
    assert!(p.get_info(&stage).is_none());
    assert!(rp.get_info(&stage).is_none());
    assert!(port.get_info(&stage).is_none());
    assert!(uses_set(&stage, x).is_empty());
    assert_eq!(outer.statements(&stage).collect::<Vec<_>>(), vec![split]);
    // UnGraph bodies are out of scope and still survive their owner.
    assert!(ug.get_info(&stage).is_some());
    assert!(iso.get_info(&stage).is_some());
    verify_derived(&stage).unwrap();
}

#[test]
fn replace_operand_on_member_retargets_its_edge() {
    let mut stage = new_stage();

    let (split, x, y) = make_split(&mut stage);
    let _outer = stage.block().stmt(split).new();
    let (p, rp) = make_compute(&mut stage, x);
    let (q, rq) = make_compute(&mut stage, x);
    let c = stage.statement().definition(BuilderDialect::Use(rp)).new();
    let dg = stage.digraph().node(p).node(q).node(c).new();

    let mut stage = stage.finalize().unwrap();
    let operands = MutationEvent::ChangedOperands { stmt: c };
    let topology = MutationEvent::ChangedDiGraphTopology { graph: dg };
    let mut rw = Rewriter::new(&mut stage);

    // Member result → member result: the edge moves to the new producer.
    assert_eq!(rw.replace_operand(c, 0, rq), Ok(rp));
    assert_eq!(edges(rw.stage(), dg), HashMap::from([((q, c, rq), 1)]));
    assert_eq!(rw.drain_events(), vec![operands, topology]);

    // Member result → outside value: the edge goes.
    rw.replace_operand(c, 0, x).unwrap();
    assert!(edges(rw.stage(), dg).is_empty());
    assert_eq!(rw.drain_events(), vec![operands, topology]);

    // Outside → outside: no edge either way, so no topology event.
    rw.replace_operand(c, 0, y).unwrap();
    assert!(edges(rw.stage(), dg).is_empty());
    assert_eq!(rw.drain_events(), vec![operands]);

    // Outside → member result: an edge appears.
    rw.replace_operand(c, 0, rp).unwrap();
    assert_eq!(edges(rw.stage(), dg), HashMap::from([((p, c, rp), 1)]));
    assert_eq!(rw.drain_events(), vec![operands, topology]);
    drop(rw);

    assert_eq!(uses_set(&stage, rp), HashSet::from([so(c, 0)]));
    assert!(uses_set(&stage, rq).is_empty());
    verify_derived(&stage).unwrap();
}

#[test]
fn replace_all_uses_retargets_every_member_edge() {
    let mut stage = new_stage();

    // `rp` is read by two members (one of them twice) and by the yield.
    let (split, x, _) = make_split(&mut stage);
    let _outer = stage.block().stmt(split).new();
    let (p, rp) = make_compute(&mut stage, x);
    let (q, rq) = make_compute(&mut stage, x);
    let c1 = stage.statement().definition(BuilderDialect::Use(rp)).new();
    let c2 = stage
        .statement()
        .definition(BuilderDialect::Add(rp, rp))
        .new();
    let dg = stage
        .digraph()
        .node(p)
        .node(q)
        .node(c1)
        .node(c2)
        .yield_value(rp)
        .new();

    let mut stage = stage.finalize().unwrap();
    let mut rw = Rewriter::new(&mut stage);
    assert_eq!(rw.replace_all_uses(rp, rq), Ok(4));
    let events = rw.drain_events();

    // One topology and one yields event for the graph, however many slots.
    let expected = [
        MutationEvent::ChangedOperands { stmt: c1 },
        MutationEvent::ChangedOperands { stmt: c2 },
        MutationEvent::ChangedDiGraphTopology { graph: dg },
        MutationEvent::ChangedDiGraphYields { graph: dg },
        MutationEvent::ReplacedUses { old: rp, new: rq },
    ];
    assert_eq!(events.len(), expected.len());
    assert_eq!(
        events.iter().copied().collect::<HashSet<_>>(),
        HashSet::from(expected)
    );

    // Every edge out of `p` now leaves `q`, and `p` is free to go.
    assert_eq!(
        edges(rw.stage(), dg),
        HashMap::from([((q, c1, rq), 1), ((q, c2, rq), 2)])
    );
    rw.erase_statement(p).unwrap();
    drop(rw);

    assert_members(&stage, dg, &[q, c1, c2]);
    assert_eq!(
        uses_set(&stage, rq),
        HashSet::from([so(c1, 0), so(c2, 0), so(c2, 1), dgy(dg, 0)])
    );
    verify_derived(&stage).unwrap();
}

#[test]
fn replace_statement_on_member_updates_operand_edges() {
    let mut stage = new_stage();

    // p → n → d, with q a second producer.
    let (split, x, _) = make_split(&mut stage);
    let _outer = stage.block().stmt(split).new();
    let (p, rp) = make_compute(&mut stage, x);
    let (q, rq) = make_compute(&mut stage, x);
    let (n, rn) = make_compute(&mut stage, rp);
    let d = stage.statement().definition(BuilderDialect::Use(rn)).new();
    let dg = stage.digraph().node(p).node(q).node(n).node(d).new();

    let mut stage = stage.finalize().unwrap();
    // The result slot is overwritten with `n`'s own result on replacement.
    let placeholder = ResultValue::from(TestSSAValue(0));
    let mut rw = Rewriter::new(&mut stage);

    // A member cannot become a terminator.
    assert_eq!(
        rw.replace_statement(n, BuilderDialect::Invoke(placeholder)),
        Err(RewriteError::TerminatorKindMismatch(n))
    );
    assert!(rw.events().is_empty());

    // Reading `rq` instead of `rp` swaps the incoming edge; the result keeps
    // its id, so the outgoing edge to `d` stands.
    rw.replace_statement(n, BuilderDialect::Compute(rq, placeholder))
        .unwrap();
    assert_eq!(
        rw.drain_events(),
        vec![
            MutationEvent::ReplacedStatement { stmt: n },
            MutationEvent::ChangedDiGraphTopology { graph: dg },
        ]
    );
    assert_eq!(
        edges(rw.stage(), dg),
        HashMap::from([((q, n, rq), 1), ((n, d, rn), 1)])
    );

    // Same operands again: the topology is unchanged, so no topology event.
    rw.replace_statement(n, BuilderDialect::Compute(rq, placeholder))
        .unwrap();
    assert_eq!(
        rw.drain_events(),
        vec![MutationEvent::ReplacedStatement { stmt: n }]
    );
    drop(rw);

    assert_eq!(*rn.get_info(&stage).unwrap().kind(), SSAKind::Result(n, 0));
    assert!(uses_set(&stage, rp).is_empty());
    assert_eq!(uses_set(&stage, rq), HashSet::from([so(n, 0)]));
    verify_derived(&stage).unwrap();
}

#[test]
fn rewriter_maintains_edges_of_an_attached_digraph() {
    let mut stage = new_stage();

    // Built the way the parser builds graphs: empty, then members attached.
    let (split, x, _) = make_split(&mut stage);
    let _outer = stage.block().stmt(split).new();
    let (p, rp) = make_compute(&mut stage, x);
    let c = stage.statement().definition(BuilderDialect::Use(rp)).new();
    let dg = stage.digraph().new();
    stage.attach_nodes_to_digraph(dg, &[p, c], &[]);

    let mut stage = stage.finalize().unwrap();
    assert_members(&stage, dg, &[p, c]);
    assert_eq!(edges(&stage, dg), HashMap::from([((p, c, rp), 1)]));

    let mut rw = Rewriter::new(&mut stage);
    rw.erase_statement(c).unwrap();
    drop(rw);

    assert_members(&stage, dg, &[p]);
    assert!(edges(&stage, dg).is_empty());
    verify_derived(&stage).unwrap();
}
