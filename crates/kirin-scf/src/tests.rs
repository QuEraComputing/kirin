use kirin::ir::{
    HasArguments, HasBlocks, HasRegions, HasResults, HasSuccessors, IsConstant, IsPure,
    IsSpeculatable, IsTerminator, TestSSAValue,
};
use kirin_test_types::UnitType;

use crate::{StructuredControlFlow, Yield};

fn make_yield() -> Yield<UnitType> {
    Yield {
        values: vec![TestSSAValue(0).into()],
        marker: std::marker::PhantomData,
    }
}

fn make_void_yield() -> Yield<UnitType> {
    Yield {
        values: vec![],
        marker: std::marker::PhantomData,
    }
}

fn make_scf_yield() -> StructuredControlFlow<UnitType> {
    StructuredControlFlow::Yield(make_yield())
}

// --- Yield: is a terminator ---

#[test]
fn yield_is_terminator() {
    assert!(make_yield().is_terminator());
}

#[test]
fn scf_yield_is_terminator() {
    assert!(make_scf_yield().is_terminator());
}

// --- Yield: not pure, not constant, not speculatable ---

#[test]
fn yield_not_pure() {
    assert!(!make_yield().is_pure());
}

#[test]
fn yield_not_constant() {
    assert!(!make_yield().is_constant());
}

#[test]
fn yield_not_speculatable() {
    assert!(!make_yield().is_speculatable());
}

// --- Yield: has arguments (the values), no results, no successors ---

#[test]
fn yield_has_one_argument() {
    let y = make_yield();
    let args: Vec<_> = y.arguments().copied().collect();
    assert_eq!(args.len(), 1);
    assert_eq!(args[0], TestSSAValue(0).into());
}

#[test]
fn void_yield_has_no_arguments() {
    let y = make_void_yield();
    assert_eq!(y.arguments().count(), 0);
}

#[test]
fn void_yield_is_terminator() {
    assert!(make_void_yield().is_terminator());
}

#[test]
fn yield_no_results() {
    assert_eq!(make_yield().results().count(), 0);
}

#[test]
fn yield_no_successors() {
    assert_eq!(make_yield().successors().count(), 0);
}

#[test]
fn yield_no_blocks() {
    assert_eq!(make_yield().blocks().count(), 0);
}

#[test]
fn yield_no_regions() {
    assert_eq!(make_yield().regions().count(), 0);
}

// --- Clone + PartialEq for Yield ---

#[test]
fn yield_clone_eq() {
    let y = make_yield();
    assert_eq!(y, y.clone());
}

// --- Debug formatting ---

#[test]
fn yield_debug() {
    let dbg = format!("{:?}", make_yield());
    assert!(dbg.contains("Yield"), "debug should contain 'Yield': {dbg}");
}

// --- StructuredControlFlow wraps delegation: Yield variant ---

#[test]
fn scf_yield_arguments() {
    let scf = make_scf_yield();
    assert_eq!(scf.arguments().count(), 1);
}

#[test]
fn scf_yield_no_results() {
    assert_eq!(make_scf_yield().results().count(), 0);
}

#[test]
fn scf_yield_no_successors() {
    assert_eq!(make_scf_yield().successors().count(), 0);
}

#[test]
fn scf_yield_debug() {
    let dbg = format!("{:?}", make_scf_yield());
    assert!(dbg.contains("Yield"), "debug should contain 'Yield': {dbg}");
}
