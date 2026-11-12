//! The driver's worklist: which statements a sweep visits, and how often.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use kirin::prelude::*;
use kirin_arith::Arith;
use kirin_ir::walk::{
    BFS, KeepOrderReverseBFS, PostOrder, PreOrder, ReverseBFS, WalkOrder, WalkResult,
};
use kirin_rewrite::{
    DriverError, PatternRule, RewriteDriver, RewriteSummary, RuleSet, StatementRule,
};
use kirin_test_languages::ArithFunctionLanguage as L;
use kirin_wildcard::rewrite_rule;

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

/// `x` negated six times: three double negations, each folding only once
/// the one before it has.
const SIX_NEGS: &str = "stage @arith fn @f(i64) -> i64;
specialize @arith fn @f(i64) -> i64 cfg {
  ^entry(%x: i64) {
    %a = neg %x -> i64;
    %b = neg %a -> i64;
    %c = neg %b -> i64;
    %d = neg %c -> i64;
    %e = neg %d -> i64;
    %f = neg %e -> i64;
    ret %f;
  }
}
";

/// What [`SIX_NEGS`] folds to.
const IDENTITY: &str = "stage @arith fn @f(i64) -> i64;
specialize @arith fn @f(i64) -> i64 cfg {
  ^entry(%x: i64) {
    ret %x;
  }
}
";

/// An `add` of the two arguments.
const ADD: &str = "stage @arith fn @f(i64, i64) -> i64;
specialize @arith fn @f(i64, i64) -> i64 cfg {
  ^entry(%x: i64, %y: i64) {
    %s = add %x, %y -> i64;
    ret %s;
  }
}
";

/// For each sweep, how many times each statement was visited.
type Visits = Rc<RefCell<Vec<HashMap<Statement, usize>>>>;

/// Counts the visits of each statement in its sweep's entry of `visits`,
/// editing nothing.
struct CountVisits {
    visits: Visits,
    sweep: usize,
}

impl CountVisits {
    /// The copy for a new sweep, with an entry of its own.
    fn new(visits: &Visits) -> Self {
        let sweep = visits.borrow().len();
        visits.borrow_mut().push(HashMap::new());
        Self {
            visits: Rc::clone(visits),
            sweep,
        }
    }
}

impl StatementRule<L> for CountVisits {
    fn apply(
        &mut self,
        _rewriter: &mut Rewriter<'_, L>,
        stmt: Statement,
    ) -> Result<WalkResult, RewriteError> {
        *self.visits.borrow_mut()[self.sweep]
            .entry(stmt)
            .or_default() += 1;
        Ok(WalkResult::Advance)
    }
}

/// Swaps the operands of every `add`, which changes it at every visit.
struct SwapAddOperands;

impl StatementRule<L> for SwapAddOperands {
    fn apply(
        &mut self,
        rewriter: &mut Rewriter<'_, L>,
        stmt: Statement,
    ) -> Result<WalkResult, RewriteError> {
        if let L::Arith(Arith::Add { lhs, rhs, .. }) =
            *stmt.expect_info(rewriter.stage()).definition()
        {
            rewriter.replace_operand(stmt, 0, rhs)?;
            rewriter.replace_operand(stmt, 1, lhs)?;
        }
        Ok(WalkResult::Advance)
    }
}

/// `input`, parsed and printed back.
fn printed(input: &str) -> String {
    let mut pipeline: Pipeline<StageInfo<L>> = Pipeline::new();
    pipeline.parse(input).expect("the program parses");
    pipeline.sprint()
}

/// Run `driver` over `input`: the printed result and the outcome, and the
/// statements under the function, in pre-order, before the run.
fn run<O: WalkOrder>(
    driver: &RewriteDriver<'_, L, O>,
    input: &str,
) -> (
    String,
    Result<RewriteSummary, DriverError>,
    Vec<Statement>,
    StageStatus,
) {
    let mut pipeline: Pipeline<StageInfo<L>> = Pipeline::new();
    pipeline.parse(input).expect("the program parses");
    let stage = pipeline.stage_by_name("arith").expect("the stage exists");
    let mut outcome = None;
    let mut statements = Vec::new();
    let _ = pipeline.run_pass(stage, |rewriter| {
        let root = rewriter
            .stage()
            .staged_function_arena()
            .iter()
            .flat_map(|function| function.specializations())
            .map(|specialization| *specialization.definition())
            .next()
            .expect("the program has a function");
        statements = PreOrder::schedule(rewriter.stage(), root);
        let result = driver.run(rewriter);
        outcome = Some(result.clone());
        result
    });
    let status = pipeline.stage_status(stage).expect("the stage exists");
    (
        pipeline.sprint(),
        outcome.expect("the pass ran"),
        statements,
        status,
    )
}

/// Each fold changes the next `neg`, which is still in the queue, so one
/// sweep folds the whole chain in every order, and a second finds nothing.
#[test]
fn a_chain_of_folds_finishes_in_one_sweep_in_every_order() {
    fn check<O: WalkOrder>(order: &str) {
        let rules = RuleSet::build(|stage| vec![double_neg(stage)]).unwrap();
        let driver = RewriteDriver::new(&rules).walk_order::<O>();
        let (output, summary, _, _) = run(&driver, SIX_NEGS);
        assert_eq!(output, printed(IDENTITY), "{order}");
        assert_eq!(summary.unwrap().sweeps, 2, "{order}");
    }
    check::<PreOrder>("PreOrder");
    check::<PostOrder>("PostOrder");
    check::<BFS>("BFS");
    check::<ReverseBFS>("ReverseBFS");
    check::<KeepOrderReverseBFS>("KeepOrderReverseBFS");
}

/// Folding `%b` changes `%c`, and folding `%d` changes `ret`, both before
/// their turn: each stays in its place and is visited once. Their owner, the
/// function, was visited first, so each edit adds it back, once.
#[test]
fn a_statement_changed_before_its_turn_is_visited_once() {
    let visits: Visits = Rc::default();
    let mut rules = RuleSet::build(|stage| vec![double_neg(stage)]).unwrap();
    let counting = Rc::clone(&visits);
    rules.add_statement_rule("count_visits", move || CountVisits::new(&counting));
    let (_, summary, statements, _) = run(&RewriteDriver::new(&rules), FOUR_NEGS);
    assert_eq!(summary.unwrap().sweeps, 2);

    let [function, a, b, c, d, ret] = statements[..] else {
        panic!("the function has five statements");
    };
    let first_sweep = &visits.borrow()[0];
    assert_eq!(first_sweep[&function], 2);
    for stmt in [a, b, c, d, ret] {
        assert_eq!(first_sweep[&stmt], 1);
    }
}

/// A rule changing a statement at every visit adds it back every time, so
/// the sweep never ends on its own.
#[test]
fn a_sweep_rewriting_without_end_fails_the_run() {
    let mut rules = RuleSet::build(|_| Vec::<PatternRule<L>>::new()).unwrap();
    rules.add_statement_rule("swap_add_operands", || SwapAddOperands);
    let driver = RewriteDriver::new(&rules).max_rewrites_per_sweep(10);
    let (_, outcome, statements, status) = run(&driver, ADD);
    assert_eq!(
        outcome,
        Err(DriverError::RewritesExhausted {
            root: statements[0],
            rewrites: 11,
        })
    );
    assert_eq!(status, StageStatus::Poisoned);
}

/// [`FOUR_NEGS`] after `run_at` on `%b` alone: `%b`'s fold changes `%c`,
/// which is not under `%b`, so `%d`, which would fold now, is left as is.
const FOLDED_AT_B: &str = "stage @arith fn @f(i64) -> i64;
specialize @arith fn @f(i64) -> i64 cfg {
  ^entry(%x: i64) {
    %a = neg %x -> i64;
    %b = neg %a -> i64;
    %c = neg %x -> i64;
    %d = neg %c -> i64;
    ret %d;
  }
}
";

/// An edit under the root names statements outside it, here `%c`, its owner
/// and its user `%d`: none of them is visited.
#[test]
fn run_at_visits_only_the_tree_under_its_root() {
    let visits: Visits = Rc::default();
    let mut rules = RuleSet::build(|stage| vec![double_neg(stage)]).unwrap();
    let counting = Rc::clone(&visits);
    rules.add_statement_rule("count_visits", move || CountVisits::new(&counting));
    let driver = RewriteDriver::new(&rules);

    let mut pipeline: Pipeline<StageInfo<L>> = Pipeline::new();
    pipeline.parse(FOUR_NEGS).expect("the program parses");
    let stage = pipeline.stage_by_name("arith").expect("the stage exists");
    let mut b = None;
    pipeline
        .run_pass(stage, |rewriter| {
            let root = rewriter
                .stage()
                .staged_function_arena()
                .iter()
                .flat_map(|function| function.specializations())
                .map(|specialization| *specialization.definition())
                .next()
                .expect("the program has a function");
            let [_, _, stmt, ..] = PreOrder::schedule(rewriter.stage(), root)[..] else {
                panic!("the function has statements");
            };
            b = Some(stmt);
            driver.run_at(rewriter, stmt)
        })
        .expect("the pass succeeds");

    assert_eq!(pipeline.sprint(), printed(FOLDED_AT_B));
    let b = b.expect("the pass ran");
    for sweep in visits.borrow().iter() {
        assert_eq!(sweep.keys().copied().collect::<Vec<_>>(), vec![b]);
        assert_eq!(sweep[&b], 1);
    }
}
