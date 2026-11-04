//! Native statement rules in the driver's sweeps: a fresh copy each sweep,
//! tried in list order with pattern rules.

use kirin::prelude::*;
use kirin_arith::Arith;
use kirin_ir::SSAKind;
use kirin_ir::walk::{PreOrder, WalkResult};
use kirin_rewrite::{
    DriverError, PatternRule, RewriteDriver, RewriteSummary, RuleSet, StatementRule,
};
use kirin_test_languages::ArithFunctionLanguage as L;
use kirin_wildcard::{PatternLanguage, rewrite_rule};

/// `-(-x)` is `x`.
#[rewrite_rule(language = L)]
fn double_neg() {
    let a = op![Arith::neg(x)];
    let r = op![Arith::neg(a)];
    rewrite!(r, {
        replace!(r, x);
    })
}

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

/// If `stmt` is `%r = neg %a` with `%a = neg %x`, `%r` and `%x`.
fn double_neg_at(stage: &StageInfo<L>, stmt: Statement) -> Option<(SSAValue, SSAValue)> {
    let L::Arith(Arith::Neg { operand, result }) = stmt.expect_info(stage).definition() else {
        return None;
    };
    let SSAKind::Result(producer, _) = *operand.expect_info(stage).kind() else {
        return None;
    };
    let L::Arith(Arith::Neg { operand: x, .. }) = producer.expect_info(stage).definition() else {
        return None;
    };
    Some(((*result).into(), *x))
}

/// Folds the first double negation it visits, then no other: a copy kept
/// across sweeps would fold only one in the whole run.
#[derive(Default)]
struct FoldOnce {
    done: bool,
}

impl StatementRule<L> for FoldOnce {
    fn apply(
        &mut self,
        rewriter: &mut Rewriter<'_, L>,
        stmt: Statement,
    ) -> Result<WalkResult, RewriteError> {
        if self.done {
            return Ok(WalkResult::Advance);
        }
        if let Some((r, x)) = double_neg_at(rewriter.stage(), stmt) {
            rewriter.replace_all_uses(r, x)?;
            self.done = true;
        }
        Ok(WalkResult::Advance)
    }
}

/// Ends the sweep at a double negation, where `double_neg` matches, without
/// editing anything.
struct InterruptAtDoubleNeg;

impl StatementRule<L> for InterruptAtDoubleNeg {
    fn apply(
        &mut self,
        rewriter: &mut Rewriter<'_, L>,
        stmt: Statement,
    ) -> Result<WalkResult, RewriteError> {
        Ok(match double_neg_at(rewriter.stage(), stmt) {
            Some(_) => WalkResult::Interrupt,
            None => WalkResult::Advance,
        })
    }
}

/// Erases the first `neg` it visits, which the rewriter refuses while the
/// `neg`'s result is used.
struct EraseUsedNeg;

impl StatementRule<L> for EraseUsedNeg {
    fn apply(
        &mut self,
        rewriter: &mut Rewriter<'_, L>,
        stmt: Statement,
    ) -> Result<WalkResult, RewriteError> {
        if let L::Arith(Arith::Neg { .. }) = stmt.expect_info(rewriter.stage()).definition() {
            rewriter.erase_statement(stmt)?;
        }
        Ok(WalkResult::Advance)
    }
}

/// A rule set with no pattern rules yet.
fn no_patterns() -> RuleSet<L> {
    RuleSet::build(|_| Vec::<PatternRule<L>>::new()).expect("an empty rule set is valid")
}

/// An empty rule set whose pattern stage holds `double_neg`'s pattern, and
/// the `double_neg` rule, not added yet, so a test can add it before or after
/// other rules.
fn with_double_neg() -> (RuleSet<L>, PatternRule<L>) {
    let mut stage = BuilderStageInfo::<PatternLanguage<L>>::default();
    let rule = double_neg(&mut stage);
    let stage = stage.finalize().expect("the patterns finalize");
    (RuleSet::new(stage), rule)
}

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

/// What `double_neg` alone rewrites [`FOUR_NEGS`] to.
fn folded() -> String {
    let rules = RuleSet::build(|stage| vec![double_neg(stage)]).expect("the rule is valid");
    run(&rules).expect("the run succeeds").0
}

/// Each sweep's fresh copy folds one more double negation, until a sweep
/// finds none.
#[test]
fn a_statement_rule_is_fresh_each_sweep() {
    let mut rules = no_patterns();
    rules.add_statement_rule("fold_once", FoldOnce::default);
    let (output, summary) = run(&rules).unwrap();
    assert_eq!(output, folded());
    assert_eq!(summary.sweeps, 3);
    assert_eq!(summary.applied, 2);
}

/// At a double negation, the rule added first decides: interrupting before
/// `double_neg` folds it ends the run unchanged, interrupting after still
/// folds both.
#[test]
fn statement_rules_run_in_list_order_with_patterns() {
    let (mut rules, rule) = with_double_neg();
    rules.add_statement_rule("interrupt", || InterruptAtDoubleNeg);
    rules.add_pattern_rule(rule).unwrap();
    let (output, summary) = run(&rules).unwrap();
    assert_eq!(output, {
        let mut pipeline: Pipeline<StageInfo<L>> = Pipeline::new();
        pipeline.parse(FOUR_NEGS).expect("the program parses");
        pipeline.sprint()
    });
    assert_eq!(summary.sweeps, 1);
    assert_eq!(summary.applied, 0);

    let (mut rules, rule) = with_double_neg();
    rules.add_pattern_rule(rule).unwrap();
    rules.add_statement_rule("interrupt", || InterruptAtDoubleNeg);
    let (output, summary) = run(&rules).unwrap();
    assert_eq!(output, folded());
    assert_eq!(summary.sweeps, 3);
    assert_eq!(summary.applied, 2);
}

#[test]
fn a_failing_statement_rule_fails_the_run_with_its_name() {
    let mut rules = no_patterns();
    rules.add_statement_rule("erase_used_neg", || EraseUsedNeg);
    let Err(DriverError::Rewrite { rule, stmt, error }) = run(&rules) else {
        panic!("the run fails with a rewrite error");
    };
    assert_eq!(rule, "erase_used_neg");
    assert_eq!(error, RewriteError::StatementResultsInUse(stmt));
}
