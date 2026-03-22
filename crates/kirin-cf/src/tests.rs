use kirin::ir::{
    Block, HasArguments, HasBlocks, HasRegions, HasResults, HasSuccessors, IsConstant, IsPure,
    IsSpeculatable, IsTerminator, Successor, TestSSAValue,
};
use kirin_test_types::UnitType;

use crate::ControlFlow;

fn make_branch() -> ControlFlow<UnitType> {
    ControlFlow::Branch {
        target: Successor::from_block(Block::from(kirin::ir::Id::from(TestSSAValue(10)))),
        args: vec![TestSSAValue(0).into(), TestSSAValue(1).into()],
    }
}

fn make_branch_no_args() -> ControlFlow<UnitType> {
    ControlFlow::Branch {
        target: Successor::from_block(Block::from(kirin::ir::Id::from(TestSSAValue(10)))),
        args: vec![],
    }
}

fn make_cond_branch() -> ControlFlow<UnitType> {
    ControlFlow::ConditionalBranch {
        condition: TestSSAValue(0).into(),
        true_target: Successor::from_block(Block::from(kirin::ir::Id::from(TestSSAValue(10)))),
        true_args: vec![TestSSAValue(1).into()],
        false_target: Successor::from_block(Block::from(kirin::ir::Id::from(TestSSAValue(20)))),
        false_args: vec![TestSSAValue(2).into(), TestSSAValue(3).into()],
    }
}

fn make_cond_branch_no_args() -> ControlFlow<UnitType> {
    ControlFlow::ConditionalBranch {
        condition: TestSSAValue(0).into(),
        true_target: Successor::from_block(Block::from(kirin::ir::Id::from(TestSSAValue(10)))),
        true_args: vec![],
        false_target: Successor::from_block(Block::from(kirin::ir::Id::from(TestSSAValue(20)))),
        false_args: vec![],
    }
}

// --- Dialect property: both variants are terminators ---

#[test]
fn branch_is_terminator() {
    assert!(make_branch().is_terminator());
}

#[test]
fn cond_branch_is_terminator() {
    assert!(make_cond_branch().is_terminator());
}

// --- Dialect property: not pure, not constant, not speculatable ---

#[test]
fn not_pure() {
    assert!(!make_branch().is_pure());
    assert!(!make_cond_branch().is_pure());
}

#[test]
fn not_constant() {
    assert!(!make_branch().is_constant());
    assert!(!make_cond_branch().is_constant());
}

#[test]
fn not_speculatable() {
    assert!(!make_branch().is_speculatable());
    assert!(!make_cond_branch().is_speculatable());
}

// --- HasArguments ---

#[test]
fn branch_arguments_are_the_block_args() {
    let br = make_branch();
    let args: Vec<_> = br.arguments().copied().collect();
    assert_eq!(args.len(), 2);
    assert_eq!(args[0], TestSSAValue(0).into());
    assert_eq!(args[1], TestSSAValue(1).into());
}

#[test]
fn branch_no_args_has_empty_arguments() {
    let br = make_branch_no_args();
    assert_eq!(br.arguments().count(), 0);
}

#[test]
fn cond_branch_arguments_include_condition_and_all_block_args() {
    let cbr = make_cond_branch();
    let args: Vec<_> = cbr.arguments().copied().collect();
    // condition + true_args + false_args
    assert_eq!(args.len(), 4);
    assert_eq!(args[0], TestSSAValue(0).into()); // condition
    assert_eq!(args[1], TestSSAValue(1).into()); // true_args[0]
    assert_eq!(args[2], TestSSAValue(2).into()); // false_args[0]
    assert_eq!(args[3], TestSSAValue(3).into()); // false_args[1]
}

#[test]
fn cond_branch_no_args_has_only_condition() {
    let cbr = make_cond_branch_no_args();
    let args: Vec<_> = cbr.arguments().copied().collect();
    assert_eq!(args.len(), 1);
    assert_eq!(args[0], TestSSAValue(0).into()); // condition only
}

// --- HasResults: terminators have no results ---

#[test]
fn no_results() {
    assert_eq!(make_branch().results().count(), 0);
    assert_eq!(make_cond_branch().results().count(), 0);
}

// --- HasSuccessors ---

#[test]
fn branch_has_one_successor() {
    let br = make_branch();
    let succs: Vec<_> = br.successors().collect();
    assert_eq!(succs.len(), 1);
}

#[test]
fn cond_branch_has_two_successors() {
    let cbr = make_cond_branch();
    let succs: Vec<_> = cbr.successors().collect();
    assert_eq!(succs.len(), 2);
}

// --- HasBlocks / HasRegions: empty ---

#[test]
fn no_blocks() {
    assert_eq!(make_branch().blocks().count(), 0);
    assert_eq!(make_cond_branch().blocks().count(), 0);
}

#[test]
fn no_regions() {
    assert_eq!(make_branch().regions().count(), 0);
    assert_eq!(make_cond_branch().regions().count(), 0);
}

// --- Clone + PartialEq ---

#[test]
fn clone_eq() {
    let br = make_branch();
    assert_eq!(br, br.clone());
    let cbr = make_cond_branch();
    assert_eq!(cbr, cbr.clone());
}

#[test]
fn different_variants_not_equal() {
    assert_ne!(make_branch(), make_cond_branch());
}

// --- Debug formatting ---

#[test]
fn debug_contains_variant_name() {
    assert!(format!("{:?}", make_branch()).contains("Branch"));
    assert!(format!("{:?}", make_cond_branch()).contains("ConditionalBranch"));
}

// --- Successor::target and from_block roundtrip ---

#[test]
fn successor_roundtrip() {
    let block = Block::from(kirin::ir::Id::from(TestSSAValue(42)));
    let succ = Successor::from_block(block);
    assert_eq!(succ.target(), block);
}
