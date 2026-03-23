use kirin::ir::{
    HasArguments, HasBlocks, HasRegions, HasResults, HasSuccessors, IsConstant, IsPure,
    IsSpeculatable, IsTerminator, TestSSAValue,
};
use kirin_test_types::UnitType;

use crate::{NewTuple, Tuple, Unpack};

fn make_new_tuple() -> NewTuple<UnitType> {
    NewTuple {
        args: vec![TestSSAValue(0).into(), TestSSAValue(1).into()],
        result: TestSSAValue(2).into(),
        marker: std::marker::PhantomData,
    }
}

fn make_unpack() -> Unpack<UnitType> {
    Unpack {
        source: TestSSAValue(0).into(),
        results: vec![TestSSAValue(1).into(), TestSSAValue(2).into()],
        marker: std::marker::PhantomData,
    }
}

// --- NewTuple: not a terminator ---

#[test]
fn new_tuple_not_terminator() {
    assert!(!make_new_tuple().is_terminator());
}

#[test]
fn new_tuple_not_pure() {
    assert!(!make_new_tuple().is_pure());
}

#[test]
fn new_tuple_not_constant() {
    assert!(!make_new_tuple().is_constant());
}

#[test]
fn new_tuple_not_speculatable() {
    assert!(!make_new_tuple().is_speculatable());
}

// --- NewTuple: arguments and results ---

#[test]
fn new_tuple_has_arguments() {
    let op = make_new_tuple();
    let args: Vec<_> = op.arguments().copied().collect();
    assert_eq!(args.len(), 2);
}

#[test]
fn new_tuple_has_one_result() {
    let op = make_new_tuple();
    let results: Vec<_> = op.results().copied().collect();
    assert_eq!(results.len(), 1);
}

#[test]
fn new_tuple_no_successors() {
    assert_eq!(make_new_tuple().successors().count(), 0);
}

#[test]
fn new_tuple_no_blocks() {
    assert_eq!(make_new_tuple().blocks().count(), 0);
}

#[test]
fn new_tuple_no_regions() {
    assert_eq!(make_new_tuple().regions().count(), 0);
}

// --- Unpack: not a terminator ---

#[test]
fn unpack_not_terminator() {
    assert!(!make_unpack().is_terminator());
}

#[test]
fn unpack_not_pure() {
    assert!(!make_unpack().is_pure());
}

#[test]
fn unpack_not_constant() {
    assert!(!make_unpack().is_constant());
}

#[test]
fn unpack_not_speculatable() {
    assert!(!make_unpack().is_speculatable());
}

// --- Unpack: arguments and results ---

#[test]
fn unpack_has_one_argument() {
    let op = make_unpack();
    let args: Vec<_> = op.arguments().copied().collect();
    assert_eq!(args.len(), 1);
}

#[test]
fn unpack_has_two_results() {
    let op = make_unpack();
    let results: Vec<_> = op.results().copied().collect();
    assert_eq!(results.len(), 2);
}

#[test]
fn unpack_no_successors() {
    assert_eq!(make_unpack().successors().count(), 0);
}

#[test]
fn unpack_no_blocks() {
    assert_eq!(make_unpack().blocks().count(), 0);
}

#[test]
fn unpack_no_regions() {
    assert_eq!(make_unpack().regions().count(), 0);
}

// --- Clone + PartialEq ---

#[test]
fn new_tuple_clone_eq() {
    let op = make_new_tuple();
    assert_eq!(op, op.clone());
}

#[test]
fn unpack_clone_eq() {
    let op = make_unpack();
    assert_eq!(op, op.clone());
}

// --- Debug formatting ---

#[test]
fn new_tuple_debug() {
    let dbg = format!("{:?}", make_new_tuple());
    assert!(
        dbg.contains("NewTuple"),
        "debug should contain 'NewTuple': {dbg}"
    );
}

#[test]
fn unpack_debug() {
    let dbg = format!("{:?}", make_unpack());
    assert!(
        dbg.contains("Unpack"),
        "debug should contain 'Unpack': {dbg}"
    );
}

// --- Tuple wraps delegation ---

#[test]
fn tuple_op_new_tuple_not_terminator() {
    let op = Tuple::NewTuple(make_new_tuple());
    assert!(!op.is_terminator());
}

#[test]
fn tuple_op_unpack_not_terminator() {
    let op = Tuple::Unpack(make_unpack());
    assert!(!op.is_terminator());
}

#[test]
fn tuple_op_new_tuple_arguments() {
    let op = Tuple::NewTuple(make_new_tuple());
    assert_eq!(op.arguments().count(), 2);
}

#[test]
fn tuple_op_unpack_arguments() {
    let op = Tuple::Unpack(make_unpack());
    assert_eq!(op.arguments().count(), 1);
}

#[test]
fn tuple_op_new_tuple_results() {
    let op = Tuple::NewTuple(make_new_tuple());
    assert_eq!(op.results().count(), 1);
}

#[test]
fn tuple_op_unpack_results() {
    let op = Tuple::Unpack(make_unpack());
    assert_eq!(op.results().count(), 2);
}
