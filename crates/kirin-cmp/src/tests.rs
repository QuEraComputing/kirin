use kirin::ir::{
    HasArguments, HasBlocks, HasRegions, HasResults, HasSuccessors, IsConstant, IsPure,
    IsSpeculatable, IsTerminator, TestSSAValue,
};

use crate::Cmp;

/// Helper: unit type for parameterizing Cmp.
#[derive(Debug, Clone, Hash, PartialEq, Eq, Default)]
struct UnitTy;

fn make_eq() -> Cmp<UnitTy> {
    Cmp::Eq {
        lhs: TestSSAValue(0).into(),
        rhs: TestSSAValue(1).into(),
        result: TestSSAValue(2).into(),
        marker: std::marker::PhantomData,
    }
}

fn make_ne() -> Cmp<UnitTy> {
    Cmp::Ne {
        lhs: TestSSAValue(0).into(),
        rhs: TestSSAValue(1).into(),
        result: TestSSAValue(2).into(),
        marker: std::marker::PhantomData,
    }
}

fn make_lt() -> Cmp<UnitTy> {
    Cmp::Lt {
        lhs: TestSSAValue(3).into(),
        rhs: TestSSAValue(4).into(),
        result: TestSSAValue(5).into(),
        marker: std::marker::PhantomData,
    }
}

fn make_le() -> Cmp<UnitTy> {
    Cmp::Le {
        lhs: TestSSAValue(0).into(),
        rhs: TestSSAValue(1).into(),
        result: TestSSAValue(2).into(),
        marker: std::marker::PhantomData,
    }
}

fn make_gt() -> Cmp<UnitTy> {
    Cmp::Gt {
        lhs: TestSSAValue(0).into(),
        rhs: TestSSAValue(1).into(),
        result: TestSSAValue(2).into(),
        marker: std::marker::PhantomData,
    }
}

fn make_ge() -> Cmp<UnitTy> {
    Cmp::Ge {
        lhs: TestSSAValue(0).into(),
        rhs: TestSSAValue(1).into(),
        result: TestSSAValue(2).into(),
        marker: std::marker::PhantomData,
    }
}

fn all_variants() -> Vec<Cmp<UnitTy>> {
    vec![
        make_eq(),
        make_ne(),
        make_lt(),
        make_le(),
        make_gt(),
        make_ge(),
    ]
}

// --- Dialect property: all Cmp variants are pure ---

#[test]
fn all_variants_are_pure() {
    for op in all_variants() {
        assert!(op.is_pure(), "expected {op:?} to be pure");
    }
}

// --- Dialect property: all Cmp variants are speculatable ---

#[test]
fn all_variants_are_speculatable() {
    for op in all_variants() {
        assert!(op.is_speculatable(), "expected {op:?} to be speculatable");
    }
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

// --- HasArguments: each binary op has exactly 2 arguments ---

#[test]
fn arguments_count() {
    for op in all_variants() {
        let args: Vec<_> = op.arguments().collect();
        assert_eq!(args.len(), 2, "expected 2 arguments for {op:?}");
    }
}

// --- HasResults: each op has exactly 1 result ---

#[test]
fn results_count() {
    for op in all_variants() {
        let results: Vec<_> = op.results().collect();
        assert_eq!(results.len(), 1, "expected 1 result for {op:?}");
    }
}

// --- HasSuccessors: no successors ---

#[test]
fn no_successors() {
    for op in all_variants() {
        let succs: Vec<_> = op.successors().collect();
        assert_eq!(succs.len(), 0, "expected 0 successors for {op:?}");
    }
}

// --- HasBlocks: no blocks ---

#[test]
fn no_blocks() {
    for op in all_variants() {
        let blocks: Vec<_> = op.blocks().collect();
        assert_eq!(blocks.len(), 0, "expected 0 blocks for {op:?}");
    }
}

// --- HasRegions: no regions ---

#[test]
fn no_regions() {
    for op in all_variants() {
        let regions: Vec<_> = op.regions().collect();
        assert_eq!(regions.len(), 0, "expected 0 regions for {op:?}");
    }
}

// --- Clone + PartialEq ---

#[test]
fn clone_eq() {
    let eq = make_eq();
    assert_eq!(eq, eq.clone());
}

#[test]
fn different_variants_not_equal() {
    assert_ne!(make_eq(), make_ne());
    assert_ne!(make_lt(), make_gt());
    assert_ne!(make_le(), make_ge());
}

// --- Debug formatting contains variant name ---

#[test]
fn debug_contains_variant_name() {
    assert!(format!("{:?}", make_eq()).contains("Eq"));
    assert!(format!("{:?}", make_ne()).contains("Ne"));
    assert!(format!("{:?}", make_lt()).contains("Lt"));
    assert!(format!("{:?}", make_le()).contains("Le"));
    assert!(format!("{:?}", make_gt()).contains("Gt"));
    assert!(format!("{:?}", make_ge()).contains("Ge"));
}

// --- Arguments return the correct SSA values ---

#[test]
fn arguments_are_correct_ssa_values() {
    let lt = make_lt();
    let args: Vec<_> = lt.arguments().copied().collect();
    assert_eq!(args[0], TestSSAValue(3).into());
    assert_eq!(args[1], TestSSAValue(4).into());
}

// --- Results return the correct result value ---

#[test]
fn result_is_correct() {
    let lt = make_lt();
    let results: Vec<_> = lt.results().copied().collect();
    let expected: kirin::ir::ResultValue = TestSSAValue(5).into();
    assert_eq!(results[0], expected);
}
