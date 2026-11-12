//! [`Rewriter::insert`] rejects a place nested in the statement's own bodies.
//!
//! In-crate because it needs a detached statement that owns a body, which no
//! public API makes yet.

use crate::arena::GetInfo;
use crate::testing::dialect::{TestLang, new_stage};
use crate::{Body, InsertionPoint, RewriteError, Rewriter, StatementParent};

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
    // statement would be.
    let copy = rw.build_statement(TestLang::Region(body)).unwrap();
    rw.adopt_body(Body::Block(body), copy);

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
