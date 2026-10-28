//! A rewrite body's walk result: how [`RuleSet::add`] checks it.

use kirin::prelude::*;
use kirin_arith::{Arith, ArithType};
use kirin_ir::{BuilderSSAKind, Placeholder};
use kirin_rewrite::{NativeRegistry, NativeValue, RuleErrorKind, RuleSet};
use kirin_test_languages::ArithFunctionLanguage as L;
use kirin_wildcard::{
    Hole, NativeCall, Pattern, PatternLanguage, Replace, Rewrite, Skip, WalkResultOf, rewrite_rule,
};

type Stage = BuilderStageInfo<PatternLanguage<L>>;

/// Only named by the rules; the driver calls the `decide` registered in
/// [`natives`].
fn decide(_x: i64) -> i64 {
    unreachable!("the rules only name it")
}

/// `-(-x)` is `x`, leaving the root's nested statements unwalked.
#[rewrite_rule(language = L)]
fn double_neg_then_skip() {
    let a = op![Arith::neg(x)];
    let r = op![Arith::neg(a)];
    rewrite!(r, {
        replace!(r, x);
        skip!()
    })
}

/// `-(-x)` is `x`, going on as `decide` says.
#[rewrite_rule(language = L)]
fn double_neg_then_decide() {
    let a = op![Arith::neg(x)];
    let r = op![Arith::neg(a)];
    rewrite!(r, {
        replace!(r, x);
        decide(x)
    })
}

fn natives() -> NativeRegistry<L> {
    NativeRegistry::new().action("decide", |_, _| Ok(vec![NativeValue::Bool(true)]))
}

#[test]
fn rules_ending_in_a_walk_result_are_accepted() {
    let rules = RuleSet::build(natives(), |stage| {
        vec![double_neg_then_skip(stage), double_neg_then_decide(stage)]
    });
    assert!(rules.is_ok(), "{:?}", rules.err());
}

/// A pattern `%r = neg %x` whose rewrite body replaces `%r` with the hole
/// `%x`, then runs the actions `end` builds from `%x` and ends in the
/// terminator it returns, as a rule built without `#[rewrite_rule]` could.
fn neg_pattern_ending_in(
    stage: &mut Stage,
    end: impl FnOnce(&mut Stage, SSAValue) -> (Vec<Statement>, Statement),
) -> Statement {
    let hole = Hole::<ArithType>::new(stage);
    let x = hole.result;
    let r = result(stage, stage.statement_arena().next_id());
    let neg = stage
        .statement()
        .definition(PatternLanguage::<L>::Op(L::from(Arith::new_neg(
            SSAValue::from(x),
            r,
        ))))
        .new();

    let replace = Replace::<ArithType>::new(stage, r, x).id;
    let (actions, terminator) = end(stage, x.into());
    let rw = actions
        .into_iter()
        .fold(stage.block().stmt(replace), |block, action| {
            block.stmt(action)
        })
        .terminator(terminator)
        .new();
    let rewrite = Rewrite::<ArithType>::new(stage, neg, rw).id;
    let bb = stage
        .block()
        .stmt(hole.id)
        .stmt(neg)
        .terminator(rewrite)
        .new();
    let name = stage.symbol_table_mut().intern("neg".to_string());
    Pattern::<ArithType>::new(stage, name, bb).id
}

/// A new result of `stmt`, which is about to be built.
fn result(stage: &mut Stage, stmt: Statement) -> ResultValue {
    stage
        .ssa()
        .ty(ArithType::placeholder())
        .kind(BuilderSSAKind::Result(stmt, 0))
        .new()
        .into()
}

/// `w = native_call "decide"(%x)`, and `w`.
fn decide_call(stage: &mut Stage, x: SSAValue) -> (Statement, ResultValue) {
    let w = result(stage, stage.statement_arena().next_id());
    let call = stage
        .statement()
        .definition(NativeCall::<ArithType>::new_native_call(
            "decide".to_string(),
            vec![x],
            vec![w],
        ))
        .new();
    (call, w)
}

/// The kind of the error adding `pattern` to a rule set fails with.
fn rejection(build: impl FnOnce(&mut Stage) -> Statement) -> RuleErrorKind {
    RuleSet::build(natives(), |stage| vec![build(stage)])
        .err()
        .expect("the rule is rejected")
        .kind
}

#[test]
fn hand_built_walk_results_are_accepted() {
    let rules = RuleSet::build(natives(), |stage| {
        vec![
            neg_pattern_ending_in(stage, |stage, _| (vec![], Skip::<ArithType>::new(stage).id)),
            neg_pattern_ending_in(stage, |stage, x| {
                let (call, w) = decide_call(stage, x);
                (vec![call], WalkResultOf::<ArithType>::new(stage, w).id)
            }),
        ]
    });
    assert!(rules.is_ok(), "{:?}", rules.err());
}

#[test]
fn a_walk_result_not_computed_in_the_rewrite_body_is_rejected() {
    // Reading the hole.
    let kind = rejection(|stage| {
        neg_pattern_ending_in(stage, |stage, x| {
            (vec![], WalkResultOf::<ArithType>::new(stage, x).id)
        })
    });
    assert_eq!(kind, RuleErrorKind::WalkResultWithoutNativeCall);

    // Reading a native call built outside the rewrite body.
    let kind = rejection(|stage| {
        neg_pattern_ending_in(stage, |stage, x| {
            let (_, w) = decide_call(stage, x);
            (vec![], WalkResultOf::<ArithType>::new(stage, w).id)
        })
    });
    assert_eq!(kind, RuleErrorKind::WalkResultWithoutNativeCall);
}

#[test]
fn a_rewrite_body_ending_in_another_terminator_is_rejected() {
    // A nested `rewrite`, which terminates a pattern body, not a rewrite
    // body.
    let kind = rejection(|stage| {
        neg_pattern_ending_in(stage, |stage, _| {
            let root = Hole::<ArithType>::new(stage).id;
            let empty = stage.block().new();
            (vec![], Rewrite::<ArithType>::new(stage, root, empty).id)
        })
    });
    assert_eq!(kind, RuleErrorKind::UnexpectedTerminator);
}
