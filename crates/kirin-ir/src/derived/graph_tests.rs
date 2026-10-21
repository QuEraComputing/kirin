//! Graph mirror derivation and mismatch detection.
//!
//! In-crate for the same reason as [`super::tests`]: each mismatch needs a
//! mirror forged to disagree with intact authoritative IR, which no correct
//! [`Rewriter`](crate::Rewriter) can produce.

use crate::arena::{GetInfo, GetInfoMut};
use crate::testing::dialect::{TestLang, TestType, new_stage};
use crate::{
    BuilderSSAKind, DanglingParent, DiGraph, Finding, MemberRole, Mismatch, ResolutionInfo,
    SSAValue, StageInfo, Statement, StatementParent, UnGraph, VerifyError, verify_derived,
};

/// A fresh result value for the next statement built.
fn result(stage: &mut crate::BuilderStageInfo<TestLang>) -> SSAValue {
    stage
        .ssa()
        .ty(TestType::I32)
        .kind(BuilderSSAKind::Unresolved(ResolutionInfo::Result(0)))
        .new()
}

/// A `DiGraph` where `def` produces `value` and `user` reads it: one edge.
struct DiGraphStage {
    stage: StageInfo<TestLang>,
    graph: DiGraph,
    def: Statement,
    user: Statement,
    value: SSAValue,
}

fn digraph_stage() -> DiGraphStage {
    let mut stage = new_stage();
    let value = result(&mut stage);
    let def = stage
        .statement()
        .definition(TestLang::Def(value.into()))
        .new();
    let user = stage.statement().definition(TestLang::Use(value)).new();
    let graph = stage.digraph().node(def).node(user).new();
    DiGraphStage {
        stage: stage.finalize().unwrap(),
        graph,
        def,
        user,
        value,
    }
}

/// An `UnGraph` where nodes `a` and `b` both read the wire of `wire`: one
/// edge.
struct UnGraphStage {
    stage: StageInfo<TestLang>,
    graph: UnGraph,
    wire: Statement,
    a: Statement,
    b: Statement,
    value: SSAValue,
}

fn ungraph_stage() -> UnGraphStage {
    let mut stage = new_stage();
    let value = result(&mut stage);
    let wire = stage
        .statement()
        .definition(TestLang::Wire(value.into()))
        .new();
    let a = stage.statement().definition(TestLang::Use(value)).new();
    let b = stage.statement().definition(TestLang::Use(value)).new();
    let graph = stage.ungraph().node(b).node(a).edge(wire).new();
    UnGraphStage {
        stage: stage.finalize().unwrap(),
        graph,
        wire,
        a,
        b,
        value,
    }
}

fn mismatches(stage: &StageInfo<TestLang>) -> Vec<Mismatch> {
    match verify_derived(stage) {
        Err(VerifyError::Mismatch(mismatches)) => mismatches,
        other => panic!("expected a mismatch, got {other:?}"),
    }
}

#[test]
fn finalize_installs_the_digraph_mirror() {
    let DiGraphStage {
        stage,
        graph,
        def,
        user,
        value,
    } = digraph_stage();
    let info = graph.expect_info(&stage);

    assert_eq!(info.nodes().collect::<Vec<_>>(), [def, user]);
    let edges: Vec<_> = info
        .graph()
        .edge_indices()
        .map(|edge| {
            let (a, b) = info.graph().edge_endpoints(edge).unwrap();
            (info.graph()[a], info.graph()[b], info.graph()[edge])
        })
        .collect();
    assert_eq!(edges, [(def, user, value)]);
    verify_derived(&stage).unwrap();
}

#[test]
fn finalize_installs_the_ungraph_mirror_in_id_order() {
    let UnGraphStage {
        stage,
        graph,
        wire,
        a,
        b,
        value,
    } = ungraph_stage();
    let info = graph.expect_info(&stage);

    // Built as `b, a, wire`; stored in allocation order.
    let members: Vec<_> = info
        .members()
        .map(|(stmt, member)| (stmt, member.role()))
        .collect();
    assert_eq!(
        members,
        [
            (wire, MemberRole::Edge),
            (a, MemberRole::Node),
            (b, MemberRole::Node),
        ]
    );
    assert_eq!(
        info.graph().edge_weights().copied().collect::<Vec<_>>(),
        [value]
    );
    verify_derived(&stage).unwrap();
}

#[test]
fn a_dropped_member_is_reported_as_a_mismatch() {
    let UnGraphStage {
        mut stage,
        graph,
        wire,
        a,
        b,
        ..
    } = ungraph_stage();
    // `wire` is still parented to the graph, but the map forgets it.
    assert!(
        graph
            .get_info_mut(&mut stage)
            .unwrap()
            .remove_edge_member(wire)
    );

    assert_eq!(
        mismatches(&stage),
        [Mismatch::GraphMembers {
            graph: StatementParent::UnGraph(graph),
            installed: vec![(a, MemberRole::Node), (b, MemberRole::Node)],
            derived: vec![
                (wire, MemberRole::Edge),
                (a, MemberRole::Node),
                (b, MemberRole::Node),
            ],
        }]
    );
}

#[test]
fn an_extra_edge_is_reported_as_a_mismatch() {
    let DiGraphStage {
        mut stage,
        graph,
        def,
        user,
        value,
    } = digraph_stage();
    // A second parallel edge, though `user` reads `value` in one slot only.
    graph
        .get_info_mut(&mut stage)
        .unwrap()
        .add_edge(def, user, value);

    assert_eq!(
        mismatches(&stage),
        [Mismatch::GraphEdges {
            graph: StatementParent::DiGraph(graph),
            installed: vec![(def, user, value), (def, user, value)],
            derived: vec![(def, user, value)],
        }]
    );
}

#[test]
fn undirected_edges_compare_regardless_of_endpoint_order() {
    let UnGraphStage {
        mut stage,
        graph,
        a,
        b,
        value,
        ..
    } = ungraph_stage();
    // Re-add the one edge with its endpoints swapped.
    let info = graph.get_info_mut(&mut stage).unwrap();
    assert!(info.remove_edge(a, b, value));
    info.add_edge(b, a, value);

    verify_derived(&stage).unwrap();
}

#[test]
fn a_petgraph_node_without_a_member_is_reported_as_a_mismatch() {
    let DiGraphStage {
        mut stage,
        graph,
        def,
        ..
    } = digraph_stage();
    // A second petgraph node for `def`, which the map does not point at.
    graph
        .get_info_mut(&mut stage)
        .unwrap()
        .graph_mut()
        .add_node(def);

    assert_eq!(
        mismatches(&stage),
        [Mismatch::GraphNodeIndex {
            graph: StatementParent::DiGraph(graph),
            statements: vec![def],
        }]
    );
}

#[test]
fn a_member_of_a_tombstoned_graph_is_a_dangling_parent() {
    let DiGraphStage {
        mut stage,
        graph,
        def,
        user,
        ..
    } = digraph_stage();
    let _ = stage.nodes.digraphs.delete(graph);

    let Err(VerifyError::Derive(error)) = verify_derived(&stage) else {
        panic!("a dangling graph parent is a derive error");
    };
    let parent = StatementParent::DiGraph(graph);
    assert_eq!(
        error.findings(),
        [
            Finding::DanglingParent(DanglingParent::StatementInGraph { stmt: def, parent }),
            Finding::DanglingParent(DanglingParent::StatementInGraph { stmt: user, parent }),
        ]
    );
}
