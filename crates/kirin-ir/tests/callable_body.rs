//! Focused structural projections using kirin-ir alone.
#![cfg(feature = "derive")]

mod common;

use common::{TestType, new_stage};
use kirin_ir::*;

#[derive(Clone, Debug, PartialEq, Dialect)]
#[kirin(type = TestType, crate = kirin_ir)]
struct CfgCallable {
    auxiliary: Block,
    #[kirin(callable_body)]
    implementation: CFG,
}

#[derive(Clone, Debug, PartialEq, Dialect)]
#[kirin(type = TestType, crate = kirin_ir)]
struct UnmarkedBody {
    body: CFG,
    sig: Signature<TestType>,
}

#[derive(Clone, Debug, PartialEq, Dialect)]
#[kirin(type = TestType, crate = kirin_ir)]
struct LinearCallable(#[kirin(callable_body)] Block);

#[test]
fn marked_body_is_selected_over_an_auxiliary_body() {
    let mut stage = new_stage();
    let cfg = stage.cfg().new();
    let callable = CfgCallable {
        auxiliary: stage.block().new(),
        implementation: cfg,
    };
    assert_eq!(callable.callable_body(), Some(Body::CFG(cfg)));
}

#[test]
fn unmarked_body_and_signature_do_not_imply_callability() {
    let mut stage = new_stage();
    let operation = UnmarkedBody {
        body: stage.cfg().new(),
        sig: Signature::new(vec![], TestType::I32, ()),
    };
    assert_eq!(operation.callable_body(), None);
    assert!(operation.signature().is_some());
}

#[test]
fn marked_tuple_struct_field_is_projected() {
    let mut stage = new_stage();
    let block = stage.block().new();
    assert_eq!(
        LinearCallable(block).callable_body(),
        Some(Body::Block(block))
    );
}
