//! Native rules that read analysis results: recipes get the cache, rules
//! declare what they read, and the driver checks the cache holds it.

use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;

use kirin::prelude::*;
use kirin_arith::Arith;
use kirin_ir::SSAKind;
use kirin_ir::walk::{PreOrder, WalkOrder, WalkResult};
use kirin_rewrite::{
    Analysis, AnalysisCache, AnalysisError, AnalysisKind, BlockRule, Database, DriverError,
    PatternRule, Requirement, RewriteDriver, RuleSet, RunPassError, StatementRule,
};
use kirin_test_languages::ArithFunctionLanguage as L;

type Db = Database<StageInfo<L>>;

/// `-(-(-(-x)))`: three negations of a negation.
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

/// [`FOUR_NEGS`] with every double negation folded.
const FOLDED: &str = "stage @arith fn @f(i64) -> i64;
specialize @arith fn @f(i64) -> i64 cfg {
  ^entry(%x: i64) {
    ret %x;
  }
}
";

fn parse(input: &str) -> (Program<StageInfo<L>>, CompileStage) {
    let mut program: Program<StageInfo<L>> = Program::new();
    program.parse(input).expect("the program parses");
    let stage = program.stage_by_name("arith").expect("the stage exists");
    (program, stage)
}

fn printed(input: &str) -> String {
    parse(input).0.sprint()
}

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

/// The outer `neg` of every double negation in a stage.
struct FoldTargets;

impl AnalysisKind for FoldTargets {
    type Key = CompileStage;
    type Output = Vec<Statement>;
}

impl Analysis<StageInfo<L>> for FoldTargets {
    fn run(db: &Db, stage: &CompileStage) -> Result<Vec<Statement>, AnalysisError> {
        let info = db
            .program()
            .stage(*stage)
            .ok_or(AnalysisError::UnknownStage)?;
        let roots = info
            .staged_function_arena()
            .iter()
            .flat_map(|function| function.specializations())
            .map(|specialization| *specialization.definition());
        Ok(roots
            .flat_map(|root| PreOrder::schedule(info, root))
            .filter(|&stmt| double_neg_at(info, stmt).is_some())
            .collect())
    }
}

/// Folds the double negations [`FoldTargets`] found before the pass, if
/// each is still one when visited.
struct FoldListed {
    targets: Arc<Vec<Statement>>,
}

impl StatementRule<L> for FoldListed {
    fn apply(
        &mut self,
        rewriter: &mut Rewriter<'_, L>,
        stmt: Statement,
    ) -> Result<WalkResult, RewriteError> {
        if self.targets.contains(&stmt)
            && let Some((r, x)) = double_neg_at(rewriter.stage(), stmt)
        {
            rewriter.replace_all_uses(r, x)?;
        }
        Ok(WalkResult::Advance)
    }
}

/// A rule set with `fold_listed`, which reads [`FoldTargets`] for `stage`.
fn fold_listed(stage: CompileStage) -> RuleSet<L> {
    let mut rules =
        RuleSet::build(|_| Vec::<PatternRule<L>>::new()).expect("an empty rule set is valid");
    rules
        .add_statement_rule("fold_listed", move |cache: &AnalysisCache| FoldListed {
            targets: cache.get::<FoldTargets>(&stage).expect("declared"),
        })
        .requires::<FoldTargets>(stage);
    rules
}

/// A constant, for rules that only need something to read.
struct Answer;

impl AnalysisKind for Answer {
    type Key = ();
    type Output = u32;
}

impl Analysis<StageInfo<L>> for Answer {
    fn run(_db: &Db, _key: &()) -> Result<u32, AnalysisError> {
        Ok(42)
    }
}

/// A block rule that records the [`Answer`] its recipe was built with.
struct RecordAnswer {
    answer: u32,
    seen: Rc<Cell<Option<u32>>>,
}

impl BlockRule<L> for RecordAnswer {
    fn apply(
        &mut self,
        _rewriter: &mut Rewriter<'_, L>,
        _block: Block,
    ) -> Result<WalkResult, RewriteError> {
        self.seen.set(Some(self.answer));
        Ok(WalkResult::Advance)
    }
}

#[test]
fn a_statement_rule_reads_the_result_its_pass_was_given() {
    let (program, stage) = parse(FOUR_NEGS);
    let rules = fold_listed(stage);
    let mut db = Database::new(program);
    db.register::<FoldTargets>();
    db.run_pass(stage, rules.required(), |rewriter, cache| {
        RewriteDriver::new(&rules).with_cache(cache).run(rewriter)
    })
    .unwrap();
    assert_eq!(db.program().sprint(), printed(FOLDED));
}

#[test]
fn a_block_rule_reads_the_result_its_pass_was_given() {
    let (program, stage) = parse(FOUR_NEGS);
    let seen = Rc::new(Cell::new(None));
    let mut rules =
        RuleSet::build(|_| Vec::<PatternRule<L>>::new()).expect("an empty rule set is valid");
    let recorded = Rc::clone(&seen);
    rules
        .add_block_rule("record_answer", move |cache: &AnalysisCache| RecordAnswer {
            answer: *cache.get::<Answer>(&()).expect("declared"),
            seen: Rc::clone(&recorded),
        })
        .requires::<Answer>(());
    let mut db = Database::new(program);
    db.register::<Answer>();
    db.run_pass(stage, rules.required(), |rewriter, cache| {
        RewriteDriver::new(&rules).with_cache(cache).run(rewriter)
    })
    .unwrap();
    assert_eq!(seen.get(), Some(42));
}

#[test]
fn required_lists_every_rules_requirements_in_order() {
    let (_, stage) = parse(FOUR_NEGS);
    let mut rules = fold_listed(stage);
    rules
        .add_statement_rule("reads_two", |_| FoldListed {
            targets: Arc::default(),
        })
        .requires::<Answer>(())
        .requires::<FoldTargets>(stage);
    let names: Vec<&str> = rules.required().iter().map(Requirement::name).collect();
    let fold_targets = std::any::type_name::<FoldTargets>();
    let answer = std::any::type_name::<Answer>();
    // A result two rules read is listed twice.
    assert_eq!(names, [fold_targets, answer, fold_targets]);
}

#[test]
fn a_driver_without_a_cache_fails_before_editing() {
    let (mut program, stage) = parse(FOUR_NEGS);
    let rules = fold_listed(stage);
    let info = program.stage_mut(stage).expect("the stage is live");
    let mut rewriter = Rewriter::new(info);
    let result = RewriteDriver::new(&rules).run(&mut rewriter);
    let Err(DriverError::MissingAnalysis { rule, analysis }) = result else {
        panic!("the run fails with a missing analysis, got {result:?}");
    };
    assert_eq!(rule, "fold_listed");
    assert_eq!(analysis, std::any::type_name::<FoldTargets>());
    assert!(!rewriter.edited());
}

#[test]
fn a_pass_not_requiring_what_its_rules_read_fails() {
    let (program, stage) = parse(FOUR_NEGS);
    let rules = fold_listed(stage);
    let mut db = Database::new(program);
    db.register::<FoldTargets>();
    let mut outcome = None;
    // The pass requires nothing, so the cache lacks `FoldTargets`.
    let result = db.run_pass(stage, [], |rewriter, cache| {
        let result = RewriteDriver::new(&rules).with_cache(cache).run(rewriter);
        outcome = Some(result.clone());
        result
    });
    assert!(matches!(
        outcome,
        Some(Err(DriverError::MissingAnalysis { .. }))
    ));
    assert!(matches!(result, Err(RunPassError::PassFailedOrNotRun(_))));
}
