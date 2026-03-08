use kirin::ir::{
    HasArguments, HasBlocks, HasRegions, HasResults, HasSuccessors, IsConstant, IsPure,
    IsSpeculatable, IsTerminator, TestSSAValue,
};

use crate::Bitwise;

#[derive(Debug, Clone, Hash, PartialEq, Eq, Default)]
struct UnitTy;

fn make_and() -> Bitwise<UnitTy> {
    Bitwise::And {
        lhs: TestSSAValue(0).into(),
        rhs: TestSSAValue(1).into(),
        result: TestSSAValue(2).into(),
        marker: std::marker::PhantomData,
    }
}

fn make_or() -> Bitwise<UnitTy> {
    Bitwise::Or {
        lhs: TestSSAValue(0).into(),
        rhs: TestSSAValue(1).into(),
        result: TestSSAValue(2).into(),
        marker: std::marker::PhantomData,
    }
}

fn make_xor() -> Bitwise<UnitTy> {
    Bitwise::Xor {
        lhs: TestSSAValue(0).into(),
        rhs: TestSSAValue(1).into(),
        result: TestSSAValue(2).into(),
        marker: std::marker::PhantomData,
    }
}

fn make_not() -> Bitwise<UnitTy> {
    Bitwise::Not {
        operand: TestSSAValue(0).into(),
        result: TestSSAValue(1).into(),
        marker: std::marker::PhantomData,
    }
}

fn make_shl() -> Bitwise<UnitTy> {
    Bitwise::Shl {
        lhs: TestSSAValue(0).into(),
        rhs: TestSSAValue(1).into(),
        result: TestSSAValue(2).into(),
        marker: std::marker::PhantomData,
    }
}

fn make_shr() -> Bitwise<UnitTy> {
    Bitwise::Shr {
        lhs: TestSSAValue(0).into(),
        rhs: TestSSAValue(1).into(),
        result: TestSSAValue(2).into(),
        marker: std::marker::PhantomData,
    }
}

fn all_variants() -> Vec<Bitwise<UnitTy>> {
    vec![
        make_and(),
        make_or(),
        make_xor(),
        make_not(),
        make_shl(),
        make_shr(),
    ]
}

// --- Dialect property: all variants are pure ---

#[test]
fn all_variants_are_pure() {
    for op in all_variants() {
        assert!(op.is_pure(), "expected {op:?} to be pure");
    }
}

// --- Dialect property: and/or/xor/not are speculatable, shl/shr are not ---

#[test]
fn speculatable_variants() {
    assert!(make_and().is_speculatable());
    assert!(make_or().is_speculatable());
    assert!(make_xor().is_speculatable());
    assert!(make_not().is_speculatable());
}

#[test]
fn shift_not_speculatable() {
    assert!(!make_shl().is_speculatable());
    assert!(!make_shr().is_speculatable());
}

// --- Dialect property: none are terminators ---

#[test]
fn no_variant_is_terminator() {
    for op in all_variants() {
        assert!(!op.is_terminator(), "expected {op:?} to not be terminator");
    }
}

// --- Dialect property: none are constants ---

#[test]
fn no_variant_is_constant() {
    for op in all_variants() {
        assert!(!op.is_constant(), "expected {op:?} to not be constant");
    }
}

// --- HasArguments: binary ops have 2, unary (Not) has 1 ---

#[test]
fn binary_ops_have_two_arguments() {
    for op in [make_and(), make_or(), make_xor(), make_shl(), make_shr()] {
        let count = op.arguments().count();
        assert_eq!(count, 2, "expected 2 arguments for {op:?}");
    }
}

#[test]
fn not_has_one_argument() {
    let not = make_not();
    assert_eq!(not.arguments().count(), 1);
}

// --- HasResults: all have exactly 1 result ---

#[test]
fn all_have_one_result() {
    for op in all_variants() {
        assert_eq!(op.results().count(), 1, "expected 1 result for {op:?}");
    }
}

// --- HasSuccessors / HasBlocks / HasRegions: all empty ---

#[test]
fn no_successors() {
    for op in all_variants() {
        assert_eq!(op.successors().count(), 0);
    }
}

#[test]
fn no_blocks() {
    for op in all_variants() {
        assert_eq!(op.blocks().count(), 0);
    }
}

#[test]
fn no_regions() {
    for op in all_variants() {
        assert_eq!(op.regions().count(), 0);
    }
}

// --- Clone + PartialEq ---

#[test]
fn clone_eq() {
    for op in all_variants() {
        assert_eq!(op, op.clone());
    }
}

#[test]
fn different_variants_not_equal() {
    assert_ne!(make_and(), make_or());
    assert_ne!(make_xor(), make_not());
    assert_ne!(make_shl(), make_shr());
}

// --- Debug formatting ---

#[test]
fn debug_contains_variant_name() {
    assert!(format!("{:?}", make_and()).contains("And"));
    assert!(format!("{:?}", make_or()).contains("Or"));
    assert!(format!("{:?}", make_xor()).contains("Xor"));
    assert!(format!("{:?}", make_not()).contains("Not"));
    assert!(format!("{:?}", make_shl()).contains("Shl"));
    assert!(format!("{:?}", make_shr()).contains("Shr"));
}

// --- Arguments return correct SSA values ---

#[test]
fn not_argument_is_correct() {
    let not = make_not();
    let args: Vec<_> = not.arguments().copied().collect();
    assert_eq!(args[0], TestSSAValue(0).into());
}
