//! [`Rewriter::insert`] of detached statements: it rejects a place nested in
//! the statement's own bodies, and gives a `DiGraph` producer edges to the
//! members already reading its results.
//!
//! In-crate because both need a detached statement, made by
//! [`Rewriter::build_statement`], which no public API exposes yet.

use petgraph::visit::{EdgeRef, IntoEdgeReferences};

use crate::arena::GetInfo;
use crate::testing::dialect::{TestLang, TestType, new_stage};
use crate::{
    DiGraph, InsertionPoint, RewriteError, Rewriter, SSAValue, StageInfo, Statement,
    StatementParent, verify_derived,
};

/// `graph`'s edges as `(producer, consumer, value)`, in petgraph's order.
/// Parallel edges show up as repeats.
fn edges(stage: &StageInfo<TestLang>, graph: DiGraph) -> Vec<(Statement, Statement, SSAValue)> {
    let petgraph = graph.expect_info(stage).graph();
    petgraph
        .edge_references()
        .map(|edge| {
            (
                petgraph[edge.source()],
                petgraph[edge.target()],
                *edge.weight(),
            )
        })
        .collect()
}

#[test]
fn insert_rejects_a_place_nested_in_its_own_body() {
    // `top` is outside everything. `body` is an unowned block holding `mid`,
    // which owns `inner`, which holds `deep`.
    let mut stage = new_stage();
    let top = stage.statement().definition(TestLang::Nop).new();
    let top_block = stage.block().stmt(top).new();
    let deep = stage.statement().definition(TestLang::Nop).new();
    let inner = stage.block().stmt(deep).new();
    let mid = stage.statement().definition(TestLang::Region(inner)).new();
    let body = stage.block().stmt(mid).new();
    let mut stage = stage.finalize().unwrap();

    let mut rw = Rewriter::new(&mut stage);
    // A detached statement owning `body`, as a copy of a region-holding
    // statement would be: building it makes it the parent of `body`.
    let copy = rw.build_statement(TestLang::Region(body)).unwrap();

    let into_itself = |insertion_pt| RewriteError::CannotInsert {
        insertion_pt,
        reason: Box::new(RewriteError::InsertedIntoItself(copy)),
    };
    // One level down, and two.
    let after_mid = InsertionPoint::After(mid);
    assert_eq!(rw.insert(after_mid, copy), Err(into_itself(after_mid)));
    let before_deep = InsertionPoint::Before(deep);
    assert_eq!(rw.insert(before_deep, copy), Err(into_itself(before_deep)));

    rw.insert(InsertionPoint::Before(top), copy).unwrap();
    drop(rw);
    assert_eq!(
        copy.expect_info(&stage).parent,
        Some(StatementParent::Block(top_block))
    );
}

#[test]
fn insert_into_a_digraph_connects_members_already_reading_its_results() {
    // `seed` only makes the graph non-empty; it takes no part in the edges.
    let mut stage = new_stage();
    let seed = stage.statement().definition(TestLang::Nop).new();
    let graph = stage.digraph().node(seed).new();
    let mut stage = stage.finalize().unwrap();

    let mut rw = Rewriter::new(&mut stage);
    let value = rw.new_result(TestType::I32);
    let producer = rw.build_statement(TestLang::Def(value)).unwrap();
    // `consumer` reads `value` in both slots while `producer` is still
    // detached, so creating it adds no edge.
    let consumer = rw
        .create(
            InsertionPoint::DiGraph(graph),
            TestLang::Add(value.into(), value.into()),
        )
        .unwrap();
    assert_eq!(edges(rw.stage(), graph), []);

    // Placing `producer` gives each of `consumer`'s slots its edge.
    rw.insert(InsertionPoint::DiGraph(graph), producer).unwrap();
    drop(rw);
    let edge = (producer, consumer, SSAValue::from(value));
    assert_eq!(edges(&stage, graph), [edge, edge]);
    verify_derived(&stage).unwrap();
}
