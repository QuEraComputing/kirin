//! `#[rewrite_rule]` on generic functions: one rule, written once, builds
//! its pattern for every language its bounds admit.

use std::fmt::Debug;

use kirin::prelude::*;
use kirin_arith::{Arith, ArithType};
use kirin_test_languages::{EffectLanguage, GraphFunctionLanguage};
use kirin_wildcard::{PatternLanguage, rewrite_rule};

/// `-(-x)` into `x`, for any language containing `Arith`.
#[rewrite_rule(language = L)]
fn double_neg<L>()
where
    L: From<Arith<L::Type>>,
{
    let a = op![Arith::neg(x)];
    let r = op![Arith::neg(a)];
    replace!(r, x)
}

/// Build `rule`'s pattern in a stage of its own, finalize it, and return
/// the stage with the statements of the pattern's body.
fn build<Lang: Dialect>(
    rule: impl FnOnce(&mut BuilderStageInfo<PatternLanguage<Lang>>) -> Statement,
) -> (StageInfo<PatternLanguage<Lang>>, Vec<Statement>) {
    let mut stage = BuilderStageInfo::default();
    let pattern = rule(&mut stage);
    let stage = stage.finalize().expect("the rule builds a valid pattern");
    let PatternLanguage::Wildcard(kirin_wildcard::PatternStatement::Pattern(pattern)) =
        pattern.expect_info(&stage).definition()
    else {
        panic!("expected a `wildcard.pattern`");
    };
    let body = pattern.body().statements(&stage).collect();
    (stage, body)
}

/// The one result of `stmt`.
fn result<Lang: Dialect>(stage: &StageInfo<PatternLanguage<Lang>>, stmt: Statement) -> ResultValue {
    let results: Vec<ResultValue> = stmt
        .expect_info(stage)
        .definition()
        .results()
        .copied()
        .collect();
    let [result] = results[..] else {
        panic!("expected one result");
    };
    result
}

/// Build `double_neg` for `Lang` and check that each `neg` is the op
/// `Lang` would hold for it: the `Arith` op, wrapped by `Lang`'s `From`.
fn check_double_neg<Lang>()
where
    Lang: Dialect<Type = ArithType> + From<Arith<ArithType>> + Debug,
{
    let (stage, body) = build(double_neg::<Lang>);
    let [x, a, r] = body[..] else {
        panic!("expected the hole and both `neg`s, got {body:?}");
    };
    let neg = |operand: Statement, stmt: Statement| {
        let op = Arith::new_neg(result(&stage, operand), result(&stage, stmt));
        PatternLanguage::Op(Lang::from(op))
    };
    assert_eq!(a.expect_info(&stage).definition(), &neg(x, a));
    assert_eq!(r.expect_info(&stage).definition(), &neg(a, r));
}

#[test]
fn generic_rule_builds_for_every_language_containing_its_dialect() {
    // `Arith` itself, through `From<T> for T`.
    check_double_neg::<Arith<ArithType>>();
    // A direct `#[wraps]` variant, among non-wrapper ops.
    check_double_neg::<EffectLanguage>();
    // A direct `#[wraps]` variant, among other dialects.
    check_double_neg::<GraphFunctionLanguage>();
}
