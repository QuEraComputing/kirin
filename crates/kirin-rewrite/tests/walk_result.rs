//! A rewrite body's walk result: how [`RuleSet::add`] checks it, and how
//! the driver acts on it.

use kirin::prelude::*;
use kirin_arith::{Arith, ArithType};
use kirin_ir::walk::{PreOrder, WalkResult};
use kirin_ir::{BuilderSSAKind, Placeholder};
use kirin_rewrite::{
    DriverError, NativeRegistry, NativeValue, RewriteDriver, RewriteSummary, RuleErrorKind, RuleSet,
};
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

/// `-(-x)` is `x`.
#[rewrite_rule(language = L)]
fn double_neg() {
    let a = op![Arith::neg(x)];
    let r = op![Arith::neg(a)];
    rewrite!(r, {
        replace!(r, x);
    })
}

/// `-(-x)` is `x`, ending the sweep.
#[rewrite_rule(language = L)]
fn double_neg_then_interrupt() {
    let a = op![Arith::neg(x)];
    let r = op![Arith::neg(a)];
    rewrite!(r, {
        replace!(r, x);
        interrupt!()
    })
}

/// Only named by the rules; the driver calls the `nothing` registered in
/// [`natives_deciding`].
fn nothing(_x: i64) -> bool {
    unreachable!("the rules only name it")
}

/// Matches any `neg`, changes nothing, and ends the sweep.
#[rewrite_rule(language = L)]
fn neg_unchanged_then_interrupt() {
    let r = op![Arith::neg(x)];
    rewrite!(r, {
        let _ = nothing(r);
        interrupt!()
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

/// The natives the rules call: `decide` returns `decision`, and `nothing`
/// edits nothing.
fn natives_deciding(decision: NativeValue) -> NativeRegistry<L> {
    NativeRegistry::new()
        .action("decide", move |_, _| Ok(vec![decision]))
        .action("nothing", |_, _| Ok(vec![NativeValue::Bool(false)]))
}

fn natives() -> NativeRegistry<L> {
    natives_deciding(NativeValue::WalkResult(WalkResult::Advance))
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
    let rewrite = Rewrite::<ArithType>::new(stage, 1, rw).id;
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
            let empty = stage.block().new();
            (vec![], Rewrite::<ArithType>::new(stage, 0, empty).id)
        })
    });
    assert_eq!(kind, RuleErrorKind::UnexpectedTerminator);
}

// --- the driver ---

/// `-(-(-(-x)))`: two double negations, the second only once the first is
/// gone.
const FOUR_NEGS: &str = "stage @arith fn @f(i64) -> i64;
specialize @arith fn @f(i64) -> i64 cfg {
  ^entry(%x: i64) {
    %a = neg %x -> i64;
    %b = neg %a -> i64;
    %c = neg %b -> i64;
    %d = neg %c -> i64;
    ret %d;
  }
}
";

/// Run `rules` over [`FOUR_NEGS`] in pre-order: the printed result and the
/// summary, or the error.
fn run(rules: &RuleSet<L>) -> Result<(String, RewriteSummary), DriverError> {
    let mut pipeline: Pipeline<StageInfo<L>> = Pipeline::new();
    pipeline.parse(FOUR_NEGS).expect("the program parses");
    let stage = pipeline.stage_by_name("arith").expect("the stage exists");
    let driver = RewriteDriver::new(rules);
    let mut outcome = None;
    let _ = pipeline.run_pass(stage, |rewriter| {
        let result = driver.run::<PreOrder>(rewriter);
        outcome = Some(result.clone());
        result
    });
    let summary = outcome.expect("the pass ran")?;
    Ok((pipeline.sprint(), summary))
}

fn rules(natives: NativeRegistry<L>, build: fn(&mut Stage) -> Statement) -> RuleSet<L> {
    RuleSet::build(natives, |stage| vec![build(stage)]).expect("the rule is valid")
}

/// Without an interrupt, one sweep folds both double negations; with one,
/// each sweep folds one, and the run still reaches the same fixpoint.
#[test]
fn interrupt_ends_the_sweep_and_the_run_goes_on() {
    let (advancing, advancing_summary) = run(&rules(natives(), double_neg)).unwrap();
    assert_eq!(advancing_summary.sweeps, 2);
    assert_eq!(advancing_summary.applied, 2);

    let (interrupted, summary) = run(&rules(natives(), double_neg_then_interrupt)).unwrap();
    assert_eq!(interrupted, advancing);
    assert_eq!(summary.sweeps, 3);
    assert_eq!(summary.applied, 2);
}

/// `decide` returning a walk result steers the walk like the fixed form.
#[test]
fn a_native_walk_result_steers_the_walk() {
    let interrupting = natives_deciding(NativeValue::WalkResult(WalkResult::Interrupt));
    let (_, summary) = run(&rules(interrupting, double_neg_then_decide)).unwrap();
    assert_eq!(summary.sweeps, 3);

    let advancing = natives_deciding(NativeValue::WalkResult(WalkResult::Advance));
    let (_, summary) = run(&rules(advancing, double_neg_then_decide)).unwrap();
    assert_eq!(summary.sweeps, 2);
}

#[test]
fn a_native_returning_no_walk_result_fails_the_run() {
    let rules = rules(
        natives_deciding(NativeValue::Bool(true)),
        double_neg_then_decide,
    );
    assert_eq!(
        run(&rules).err(),
        Some(DriverError::NotAWalkResult {
            rule: "double_neg_then_decide".to_string()
        })
    );
}

/// A sweep interrupted by a rule that changed nothing ends the run.
#[test]
fn an_interrupted_sweep_that_changed_nothing_ends_the_run() {
    let (_, summary) = run(&rules(natives(), neg_unchanged_then_interrupt)).unwrap();
    assert_eq!(summary.sweeps, 1);
    assert_eq!(summary.applied, 0);
}

/// A rule's root owns no bodies yet, so there is nothing to skip.
#[test]
fn skip_does_not_change_the_run_yet() {
    let advancing = run(&rules(natives(), double_neg)).unwrap();
    let skipping = run(&rules(natives(), double_neg_then_skip)).unwrap();
    assert_eq!(skipping, advancing);
}
