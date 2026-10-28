use kirin::ir::{
    Block, HasArguments, HasBlocks, HasResults, HasSuccessors, Id, IsConstant, IsPure,
    IsSpeculatable, IsTerminator, SSAValue, Symbol, TestSSAValue,
};
use kirin_test_types::UnitType;

use crate::{
    Advance, Constraint, Erase, Hole, Interrupt, NativeCall, Pattern, PatternStatement, Replace,
    Rewrite, Skip, WalkResultOf,
};

fn ssa(id: usize) -> SSAValue {
    TestSSAValue(id).into()
}

fn block(id: usize) -> Block {
    Block::from(Id::from(TestSSAValue(id)))
}

fn make_pattern() -> Pattern<UnitType> {
    Pattern::new_pattern(Symbol::from(7), block(10))
}

fn make_hole() -> Hole<UnitType> {
    Hole::new_hole(TestSSAValue(0).into())
}

fn make_native_call() -> NativeCall<UnitType> {
    NativeCall::new_native_call(
        "divmod".to_string(),
        vec![ssa(1), ssa(2)],
        vec![TestSSAValue(4).into(), TestSSAValue(5).into()],
    )
}

fn make_constraint() -> Constraint<UnitType> {
    Constraint::new_constraint(ssa(6))
}

fn make_replace() -> Replace<UnitType> {
    Replace::new_replace(ssa(1), ssa(0))
}

fn make_erase() -> Erase<UnitType> {
    Erase::new_erase(3)
}

fn make_rewrite() -> Rewrite<UnitType> {
    Rewrite::new_rewrite(3, block(20))
}

fn make_walk_result_of() -> WalkResultOf<UnitType> {
    WalkResultOf::new_walk_result_of(ssa(8))
}

fn all_statements() -> Vec<PatternStatement<UnitType>> {
    vec![
        PatternStatement::Pattern(make_pattern()),
        PatternStatement::Hole(make_hole()),
        PatternStatement::NativeCall(make_native_call()),
        PatternStatement::Constraint(make_constraint()),
        PatternStatement::Replace(make_replace()),
        PatternStatement::Erase(make_erase()),
        PatternStatement::Rewrite(make_rewrite()),
        PatternStatement::Advance(Advance::new_advance()),
        PatternStatement::Skip(Skip::new_skip()),
        PatternStatement::Interrupt(Interrupt::new_interrupt()),
        PatternStatement::WalkResultOf(make_walk_result_of()),
    ]
}

// --- Terminators: `rewrite` ends a pattern body, the walk results a
// replacement body ---

#[test]
fn rewrite_is_terminator() {
    assert!(make_rewrite().is_terminator());
}

#[test]
fn only_rewrite_and_walk_result_variants_are_terminators() {
    // Enum-level `#[wraps]` does not delegate terminator-ness, so this checks
    // the explicit `#[kirin(terminator)]` on each variant.
    for stmt in all_statements() {
        let expected = matches!(
            stmt,
            PatternStatement::Rewrite(_)
                | PatternStatement::Advance(_)
                | PatternStatement::Skip(_)
                | PatternStatement::Interrupt(_)
                | PatternStatement::WalkResultOf(_)
        );
        assert_eq!(stmt.is_terminator(), expected, "{stmt:?}");
    }
}

#[test]
fn no_statement_is_pure_constant_or_speculatable() {
    for stmt in all_statements() {
        assert!(!stmt.is_pure(), "{stmt:?}");
        assert!(!stmt.is_constant(), "{stmt:?}");
        assert!(!stmt.is_speculatable(), "{stmt:?}");
    }
}

#[test]
fn no_statement_has_successors() {
    for stmt in all_statements() {
        assert_eq!(stmt.successors().count(), 0, "{stmt:?}");
    }
}

// --- Pattern ---

#[test]
fn pattern_has_body_and_no_operands() {
    let pattern = make_pattern();
    assert_eq!(pattern.name(), Symbol::from(7));
    assert_eq!(pattern.blocks().copied().collect::<Vec<_>>(), [block(10)]);
    assert_eq!(pattern.arguments().count(), 0);
    assert_eq!(pattern.results().count(), 0);
}

// --- Hole ---

#[test]
fn hole_defines_one_result_and_takes_no_arguments() {
    let hole = make_hole();
    assert_eq!(hole.results().copied().collect::<Vec<_>>(), [hole.result()]);
    assert_eq!(hole.arguments().count(), 0);
    assert_eq!(hole.blocks().count(), 0);
}

// --- NativeCall ---

#[test]
fn native_call_takes_args_in_order_and_defines_its_results() {
    let call = make_native_call();
    assert_eq!(call.name(), "divmod");
    assert_eq!(call.args(), [ssa(1), ssa(2)]);
    assert_eq!(call.arguments().copied().collect::<Vec<_>>(), call.args());
    assert_eq!(call.results().count(), 2);
    assert_eq!(call.blocks().count(), 0);
}

// --- Constraint ---

#[test]
fn constraint_checks_one_value() {
    let constraint = make_constraint();
    assert_eq!(constraint.success(), ssa(6));
    assert_eq!(
        constraint.arguments().copied().collect::<Vec<_>>(),
        [constraint.success()]
    );
    assert_eq!(constraint.results().count(), 0);
    assert_eq!(constraint.blocks().count(), 0);
}

// --- Replace ---

#[test]
fn replace_arguments_are_matched_then_replacement() {
    let replace = make_replace();
    assert_eq!(replace.matched(), ssa(1));
    assert_eq!(replace.replacement(), ssa(0));
    assert_eq!(
        replace.arguments().copied().collect::<Vec<_>>(),
        [ssa(1), ssa(0)]
    );
    assert_eq!(replace.results().count(), 0);
}

// --- Erase ---

/// The target is a position in the pattern body, not an operand, so erase
/// uses no value.
#[test]
fn erase_names_its_target_by_position() {
    let erase = make_erase();
    assert_eq!(erase.target(), 3);
    assert_eq!(erase.arguments().count(), 0);
    assert_eq!(erase.results().count(), 0);
}

// --- Rewrite ---

/// The root is a position in the pattern body, not an operand.
#[test]
fn rewrite_has_root_position_and_body() {
    let rewrite = make_rewrite();
    assert_eq!(rewrite.root(), 3);
    assert_eq!(rewrite.arguments().count(), 0);
    assert_eq!(
        rewrite.blocks().copied().collect::<Vec<_>>(),
        [rewrite.body()]
    );
    assert_eq!(rewrite.results().count(), 0);
}

// --- Walk results ---

#[test]
fn walk_result_of_reads_one_value() {
    let walk = make_walk_result_of();
    assert_eq!(walk.value(), ssa(8));
    assert_eq!(
        walk.arguments().copied().collect::<Vec<_>>(),
        [walk.value()]
    );
    assert_eq!(walk.results().count(), 0);
}

// --- PatternStatement wraps delegation ---

#[test]
fn pattern_statement_delegates_arguments_results_and_blocks() {
    let counts: Vec<_> = all_statements()
        .iter()
        .map(|stmt| {
            (
                stmt.arguments().count(),
                stmt.results().count(),
                stmt.blocks().count(),
            )
        })
        .collect();
    // pattern, hole, native_call, constraint, replace, erase, rewrite,
    // advance, skip, interrupt, walk_result
    assert_eq!(
        counts,
        [
            (0, 0, 1),
            (0, 1, 0),
            (2, 2, 0),
            (1, 0, 0),
            (2, 0, 0),
            (0, 0, 0),
            (0, 0, 1),
            (0, 0, 0),
            (0, 0, 0),
            (0, 0, 0),
            (1, 0, 0)
        ]
    );
}

#[test]
fn pattern_statement_clone_eq() {
    for stmt in all_statements() {
        assert_eq!(stmt, stmt.clone());
    }
}
