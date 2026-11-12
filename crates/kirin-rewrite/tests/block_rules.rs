//! Native block rules in the driver's sweeps: a fresh copy for each block a
//! visited statement owns, tried in list order with the other rules.

use std::cell::Cell;
use std::collections::HashMap;
use std::rc::Rc;

use kirin::prelude::*;
use kirin_arith::Arith;
use kirin_cf::ControlFlow;
use kirin_ir::walk::{PreOrder, ReverseBFS, WalkOrder, WalkResult};
use kirin_rewrite::{
    BlockRule, DriverError, RewriteDriver, RewriteSummary, RuleSet, StatementRule,
};
use kirin_scf::StructuredControlFlow;
use kirin_test_languages::ScfFunctionLanguage as L;
use kirin_wildcard::PatternRule;

/// A function whose CFG body has two blocks, each computing `neg %x` twice:
/// in `^entry` with a statement between the two, in `^next` one after the
/// other.
const TWO_BLOCKS: &str = "stage @arith fn @f(i64) -> i64;
specialize @arith fn @f(i64) -> i64 cfg {
  ^entry(%x: i64) {
    %a = neg %x -> i64;
    %p = add %a, %a -> i64;
    %b = neg %x -> i64;
    %s = add %p, %b -> i64;
    br ^next(%s);
  }
  ^next(%y: i64) {
    %c = neg %x -> i64;
    %d = neg %x -> i64;
    %t = add %c, %d -> i64;
    %u = add %y, %t -> i64;
    ret %u;
  }
}
";

/// [`TWO_BLOCKS`] with the duplicates within each block merged.
const TWO_BLOCKS_MERGED: &str = "stage @arith fn @f(i64) -> i64;
specialize @arith fn @f(i64) -> i64 cfg {
  ^entry(%x: i64) {
    %a = neg %x -> i64;
    %p = add %a, %a -> i64;
    %s = add %p, %a -> i64;
    br ^next(%s);
  }
  ^next(%y: i64) {
    %c = neg %x -> i64;
    %t = add %c, %c -> i64;
    %u = add %y, %t -> i64;
    ret %u;
  }
}
";

/// A function with an `scf.if` whose `then` arm computes `neg %x` twice:
/// `%t0`, which nothing uses, then `%t1`, which `%s` uses.
const IF_WITH_DUPLICATES: &str = "stage @arith fn @f(i64, i64) -> i64;
specialize @arith fn @f(i64, i64) -> i64 cfg {
  ^entry(%x: i64, %cond: i64) {
    %r = if %cond then block ^then() {
      %t0 = neg %x -> i64;
      %t1 = neg %x -> i64;
      %s = add %t1, %t1 -> i64;
      yield %s;
    } else block ^else() {
      %e = neg %x -> i64;
      yield %e;
    } -> i64;
    ret %r;
  }
}
";

/// [`IF_WITH_DUPLICATES`] after pruning the arms, then merging: `%t0` was
/// erased before the merge, so `%t1` is kept.
const IF_PRUNED_THEN_MERGED: &str = "stage @arith fn @f(i64, i64) -> i64;
specialize @arith fn @f(i64, i64) -> i64 cfg {
  ^entry(%x: i64, %cond: i64) {
    %r = if %cond then block ^then() {
      %t1 = neg %x -> i64;
      %s = add %t1, %t1 -> i64;
      yield %s;
    } else block ^else() {
      %e = neg %x -> i64;
      yield %e;
    } -> i64;
    ret %r;
  }
}
";

/// [`IF_WITH_DUPLICATES`] after merging, then pruning the arms: `%t1` was
/// merged into `%t0`, then erased.
const IF_MERGED_THEN_PRUNED: &str = "stage @arith fn @f(i64, i64) -> i64;
specialize @arith fn @f(i64, i64) -> i64 cfg {
  ^entry(%x: i64, %cond: i64) {
    %r = if %cond then block ^then() {
      %t0 = neg %x -> i64;
      %s = add %t0, %t0 -> i64;
      yield %s;
    } else block ^else() {
      %e = neg %x -> i64;
      yield %e;
    } -> i64;
    ret %r;
  }
}
";

/// [`IF_WITH_DUPLICATES`] with its `scf.if` unused, so a rule may erase it.
const UNUSED_IF: &str = "stage @arith fn @f(i64, i64) -> i64;
specialize @arith fn @f(i64, i64) -> i64 cfg {
  ^entry(%x: i64, %cond: i64) {
    %r = if %cond then block ^then() {
      %t0 = neg %x -> i64;
      %t1 = neg %x -> i64;
      %s = add %t1, %t1 -> i64;
      yield %s;
    } else block ^else() {
      %e = neg %x -> i64;
      yield %e;
    } -> i64;
    ret %x;
  }
}
";

/// [`UNUSED_IF`] with the `scf.if` erased.
const IF_ERASED: &str = "stage @arith fn @f(i64, i64) -> i64;
specialize @arith fn @f(i64, i64) -> i64 cfg {
  ^entry(%x: i64, %cond: i64) {
    ret %x;
  }
}
";

/// Merges the `neg`s of one block computing the same value into the first:
/// a small common-subexpression elimination, with a table that lasts one
/// block.
#[derive(Default)]
struct DedupNeg {
    first: HashMap<SSAValue, SSAValue>,
}

impl BlockRule<L> for DedupNeg {
    fn apply(
        &mut self,
        rewriter: &mut Rewriter<'_, L>,
        block: Block,
    ) -> Result<WalkResult, RewriteError> {
        let stmts: Vec<Statement> = block.statements(rewriter.stage()).collect();
        for stmt in stmts {
            let L::Arith(Arith::Neg { operand, result }) =
                stmt.expect_info(rewriter.stage()).definition()
            else {
                continue;
            };
            let (operand, result) = (*operand, SSAValue::from(*result));
            match self.first.get(&operand) {
                Some(&first) => {
                    rewriter.replace_all_uses(result, first)?;
                }
                None => {
                    self.first.insert(operand, result);
                }
            }
        }
        Ok(WalkResult::Advance)
    }
}

/// The recipe for [`DedupNeg`], counting the copies it builds in `built`.
fn counted_dedup(built: &Rc<Cell<usize>>) -> impl Fn() -> DedupNeg + 'static {
    let built = Rc::clone(built);
    move || {
        built.set(built.get() + 1);
        DedupNeg::default()
    }
}

/// Whether `stmt` is live and none of its results is used.
fn unused(stage: &StageInfo<L>, stmt: Statement) -> bool {
    stmt.expect_info(stage)
        .definition()
        .results()
        .all(|&result| SSAValue::from(result).expect_info(stage).uses().is_empty())
}

/// At an `scf.if`, erases the unused statements of its arms.
struct PruneArms;

impl StatementRule<L> for PruneArms {
    fn apply(
        &mut self,
        rewriter: &mut Rewriter<'_, L>,
        stmt: Statement,
    ) -> Result<WalkResult, RewriteError> {
        let L::Scf(StructuredControlFlow::If(op)) = stmt.expect_info(rewriter.stage()).definition()
        else {
            return Ok(WalkResult::Advance);
        };
        for arm in [op.then_block(), op.else_block()] {
            let stmts: Vec<Statement> = arm.statements(rewriter.stage()).collect();
            for stmt in stmts {
                if unused(rewriter.stage(), stmt) {
                    rewriter.erase_statement(stmt)?;
                }
            }
        }
        Ok(WalkResult::Advance)
    }
}

/// Erases an unused `scf.if`, arms and all.
struct EraseIf;

impl StatementRule<L> for EraseIf {
    fn apply(
        &mut self,
        rewriter: &mut Rewriter<'_, L>,
        stmt: Statement,
    ) -> Result<WalkResult, RewriteError> {
        let is_if = matches!(
            stmt.expect_info(rewriter.stage()).definition(),
            L::Scf(StructuredControlFlow::If(_))
        );
        if is_if && unused(rewriter.stage(), stmt) {
            rewriter.erase_statement(stmt)?;
        }
        Ok(WalkResult::Advance)
    }
}

/// Merges the block its block branches to into it, an edit to a sibling
/// block that the [`BlockRule`] contract forbids.
struct MergeSuccessor;

impl BlockRule<L> for MergeSuccessor {
    fn apply(
        &mut self,
        rewriter: &mut Rewriter<'_, L>,
        block: Block,
    ) -> Result<WalkResult, RewriteError> {
        let terminator = block
            .terminator(rewriter.stage())
            .expect("the block is live and terminated");
        if let L::ControlFlow(ControlFlow::Branch { target, args }) =
            terminator.expect_info(rewriter.stage()).definition()
        {
            let (succ, args) = (target.target(), args.clone());
            rewriter.merge_blocks(block, succ, &args)?;
        }
        Ok(WalkResult::Advance)
    }
}

/// [`TWO_BLOCKS`] with `^next` merged into `^entry`.
const ONE_BLOCK: &str = "stage @arith fn @f(i64) -> i64;
specialize @arith fn @f(i64) -> i64 cfg {
  ^entry(%x: i64) {
    %a = neg %x -> i64;
    %p = add %a, %a -> i64;
    %b = neg %x -> i64;
    %s = add %p, %b -> i64;
    %c = neg %x -> i64;
    %d = neg %x -> i64;
    %t = add %c, %d -> i64;
    %u = add %s, %t -> i64;
    ret %u;
  }
}
";

/// Counts the statements it is handed in `visited`.
struct CountVisits(Rc<Cell<usize>>);

impl StatementRule<L> for CountVisits {
    fn apply(
        &mut self,
        _rewriter: &mut Rewriter<'_, L>,
        _stmt: Statement,
    ) -> Result<WalkResult, RewriteError> {
        self.0.set(self.0.get() + 1);
        Ok(WalkResult::Advance)
    }
}

/// Edits nothing and returns its walk result for every block.
struct Steer(WalkResult);

impl BlockRule<L> for Steer {
    fn apply(
        &mut self,
        _rewriter: &mut Rewriter<'_, L>,
        _block: Block,
    ) -> Result<WalkResult, RewriteError> {
        Ok(self.0)
    }
}

/// A rule set with no rules yet.
fn no_rules() -> RuleSet<L> {
    RuleSet::build(|_| Vec::<PatternRule<L>>::new()).expect("an empty rule set is valid")
}

/// `input`, parsed and printed back.
fn printed(input: &str) -> String {
    let mut pipeline: Pipeline<StageInfo<L>> = Pipeline::new();
    pipeline.parse(input).expect("the program parses");
    pipeline.sprint()
}

/// Run `rules` over `input` in order `O`: the printed result and the
/// summary, or the error.
fn run<O: WalkOrder>(
    rules: &RuleSet<L>,
    input: &str,
) -> Result<(String, RewriteSummary), DriverError> {
    let mut pipeline: Pipeline<StageInfo<L>> = Pipeline::new();
    pipeline.parse(input).expect("the program parses");
    let stage = pipeline.stage_by_name("arith").expect("the stage exists");
    let driver = RewriteDriver::new(rules);
    let mut outcome = None;
    let _ = pipeline.run_pass(stage, |rewriter| {
        let result = driver.run::<O>(rewriter);
        outcome = Some(result.clone());
        result
    });
    let summary = outcome.expect("the pass ran")?;
    Ok((pipeline.sprint(), summary))
}

/// Duplicates within a block merge; the same value in another block does
/// not, since each block gets a fresh table so no duplicate is found.
#[test]
fn a_block_rule_is_fresh_for_each_block() {
    let built = Rc::new(Cell::new(0));
    let mut rules = no_rules();
    rules.add_block_rule("dedup_neg", counted_dedup(&built));
    let (output, summary) = run::<PreOrder>(&rules, TWO_BLOCKS).unwrap();
    assert_eq!(output, printed(TWO_BLOCKS_MERGED));
    assert_eq!(summary.sweeps, 2);
    assert_eq!(summary.applied, 2);
    // Two blocks, in each of two sweeps.
    assert_eq!(built.get(), 4);
}

/// `[S, B]`: pruning the arms first erases `%t0`, so dedup keeps `%t1`.
/// `[B, S]`: dedup first merges `%t1` into `%t0`, then pruning erases `%t1`,
/// left unused.
#[test]
fn rules_at_an_if_see_the_edits_of_the_rules_before_them() {
    let mut rules = no_rules();
    rules.add_statement_rule("prune_arms", || PruneArms);
    rules.add_block_rule("dedup_neg", DedupNeg::default);
    let (output, _) = run::<PreOrder>(&rules, IF_WITH_DUPLICATES).unwrap();
    assert_eq!(output, printed(IF_PRUNED_THEN_MERGED));

    let mut rules = no_rules();
    rules.add_block_rule("dedup_neg", DedupNeg::default);
    rules.add_statement_rule("prune_arms", || PruneArms);
    let (output, _) = run::<PreOrder>(&rules, IF_WITH_DUPLICATES).unwrap();
    assert_eq!(output, printed(IF_MERGED_THEN_PRUNED));
}

/// `[S, B]`: erasing the `if` first leaves block rules no arms, so only the
/// function's block gets copies of the block rule.
/// `[B, S]`: the block rule runs at the `if` before it is erased, so each
/// arm gets a copy too.
#[test]
fn a_statement_rule_erasing_an_if_first_leaves_block_rules_no_arms() {
    let built = Rc::new(Cell::new(0));
    let mut rules = no_rules();
    rules.add_statement_rule("erase_if", || EraseIf);
    rules.add_block_rule("dedup_neg", counted_dedup(&built));
    let (output, _) = run::<ReverseBFS>(&rules, UNUSED_IF).unwrap();
    assert_eq!(output, printed(IF_ERASED));
    // The function's block, in each of two sweeps.
    assert_eq!(built.get(), 2);

    let built = Rc::new(Cell::new(0));
    let mut rules = no_rules();
    rules.add_block_rule("dedup_neg", counted_dedup(&built));
    rules.add_statement_rule("erase_if", || EraseIf);
    let (output, _) = run::<ReverseBFS>(&rules, UNUSED_IF).unwrap();
    assert_eq!(output, printed(IF_ERASED));
    // The two arms and the function's block, then the function's block.
    assert_eq!(built.get(), 4);
}

/// A `skip` from one block still runs the rule on the remaining sibling blocks,
/// leaves the nested statements unwalked; an `interrupt` stops at once.
#[test]
fn a_block_rules_walk_result_steers_the_sweep() {
    let built = Rc::new(Cell::new(0));
    let visited = Rc::new(Cell::new(0));
    let steering = |walk_result: WalkResult| {
        let mut rules = no_rules();
        let built = Rc::clone(&built);
        rules.add_block_rule("steer", move || {
            built.set(built.get() + 1);
            Steer(walk_result)
        });
        let visited = Rc::clone(&visited);
        rules.add_statement_rule("count_visits", move || CountVisits(Rc::clone(&visited)));
        rules
    };

    run::<PreOrder>(&steering(WalkResult::Skip), TWO_BLOCKS).unwrap();
    // Both of the function's blocks; only the function is visited.
    assert_eq!(built.get(), 2);
    assert_eq!(visited.get(), 1);

    built.set(0);
    visited.set(0);
    run::<PreOrder>(&steering(WalkResult::Interrupt), TWO_BLOCKS).unwrap();
    // Only the first block, and no rule after it at the function.
    assert_eq!(built.get(), 1);
    assert_eq!(visited.get(), 0);
}

/// A copy merging away a block listed after its own leaves it to be
/// skipped, not handed to the next copy.
#[test]
fn a_block_erased_by_an_earlier_copy_is_skipped() {
    let built = Rc::new(Cell::new(0));
    let mut rules = no_rules();
    let counter = Rc::clone(&built);
    rules.add_block_rule("merge_successor", move || {
        counter.set(counter.get() + 1);
        MergeSuccessor
    });
    let (output, summary) = run::<PreOrder>(&rules, TWO_BLOCKS).unwrap();
    assert_eq!(output, printed(ONE_BLOCK));
    assert_eq!(summary.sweeps, 2);
    // `^entry` in each of two sweeps; `^next` is gone when its turn comes.
    assert_eq!(built.get(), 2);
}
