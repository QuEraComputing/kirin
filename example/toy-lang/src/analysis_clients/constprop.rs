use std::cell::Cell;
use std::marker::PhantomData;
use std::sync::Arc;

use kirin::ir::walk::WalkResult;
use kirin::prelude::*;
use kirin_arith::{ArithType, ArithValue};
use kirin_constant::Constant;
use kirin_constprop::{CallCtx, ConstPropValue};
use kirin_interpreter::{AnalysisResults, Callee, CrossStageLinker, ForwardEval, LinkTarget};
use kirin_rewrite::{
    Analysis, AnalysisCache, AnalysisError, AnalysisKind, Database, PatternRule, RewriteDriver,
    RuleSet, StatementRule,
};

use super::common::{helper_result, new_database, printed, roots};
use crate::interpreter::ToyConstProp;
use crate::language::LowLevel;
use crate::stage::Stage;

type Value = ConstPropValue<i64>;
/// One function's constprop result.
type Results = AnalysisResults<(LinkTarget, CallCtx<i64>), SSAValue, Value, ForwardEval>;

thread_local! {
    /// How many times [`ConstPropFacts::run`] has run on this test's thread.
    static NUM_RUNS: Cell<usize> = const { Cell::new(0) };
}

/// Constprop over a whole stage, rooting every function with unknown
/// arguments, so every function counts as an entry point.
struct ConstPropFacts;

/// The constprop result of each function of a stage.
struct StageConstants(Vec<Results>);

impl StageConstants {
    /// The join of `value`'s fact at `stage` over every function's result.
    /// `None` means no result reached `value`.
    fn value_at(&self, stage: CompileStage, value: SSAValue) -> Option<Value> {
        self.0
            .iter()
            .filter_map(|results| results.joined_fact_at(stage, value))
            .reduce(|joined, fact| joined.join(&fact))
    }
}

impl AnalysisKind for ConstPropFacts {
    type Key = CompileStage;
    type Output = StageConstants;
}

impl Analysis<Stage> for ConstPropFacts {
    fn run(db: &Database<Stage>, stage: &CompileStage) -> Result<StageConstants, AnalysisError> {
        NUM_RUNS.set(NUM_RUNS.get() + 1);
        let program = db.program();
        let roots = match program.stage(*stage).ok_or(AnalysisError::UnknownStage)? {
            Stage::Source(info) => roots(info),
            Stage::Lowered(info) => roots(info),
        };
        let mut results = Vec::new();
        for (specialization, arity) in roots {
            let engine: ToyConstProp<'_, i64> =
                ToyConstProp::new(program).with_linker(CrossStageLinker);
            let analyzed = engine
                .analyze(
                    *stage,
                    Callee::Specialized(specialization),
                    (0..arity).map(|_| Value::Top),
                )
                .map_err(AnalysisError::failed::<Self>)?;
            results.push(analyzed);
        }
        Ok(StageConstants(results))
    }
}

/// Replaces the result of an `arith` statement that constprop found constant
/// with a new `constant` inserted before it. The driver's dead-code cleanup
/// then erases the statement.
struct FoldConstants {
    stage: CompileStage,
    facts: Arc<StageConstants>,
}

impl StatementRule<LowLevel> for FoldConstants {
    fn apply(
        &mut self,
        rewriter: &mut Rewriter<'_, LowLevel>,
        stmt: Statement,
    ) -> Result<WalkResult, RewriteError> {
        let definition = stmt.expect_info(rewriter.stage()).definition();
        if !matches!(definition, LowLevel::Arith(_)) {
            return Ok(WalkResult::Advance);
        }
        let result: SSAValue = (*definition.results().next().expect("arith has a result")).into();
        // Constprop runs over `i64`, so its constants say nothing about a
        // result of another type.
        if *result.expect_info(rewriter.stage()).ty() != ArithType::I64 {
            return Ok(WalkResult::Advance);
        }
        let Some(ConstPropValue::Const(c)) = self.facts.value_at(self.stage, result) else {
            return Ok(WalkResult::Advance);
        };
        let constant = rewriter.new_result(ArithType::I64);
        rewriter.create(
            InsertionPoint::Before(stmt),
            LowLevel::Constant(Constant {
                value: ArithValue::I64(c),
                result: constant,
                marker: PhantomData,
            }),
        )?;
        rewriter.replace_all_uses(result, SSAValue::from(constant))?;
        Ok(WalkResult::Advance)
    }
}

/// A rule set with `fold_constants`, which reads [`ConstPropFacts`] for
/// `stage`.
fn fold_constants(stage: CompileStage) -> RuleSet<LowLevel> {
    let mut rules = RuleSet::build(|_| Vec::<PatternRule<LowLevel>>::new())
        .expect("an empty rule set is valid");
    rules
        .add_statement_rule("fold_constants", move |cache: &AnalysisCache| {
            FoldConstants {
                stage,
                facts: cache.get::<ConstPropFacts>(&stage).expect("declared"),
            }
        })
        .requires::<ConstPropFacts>(stage);
    rules
}

fn database(input: &str) -> (Database<Stage>, CompileStage) {
    let (mut db, stage) = new_database(input);
    db.register::<ConstPropFacts>();
    (db, stage)
}

/// Run `fold_constants` over `stage` as one pass.
fn fold(db: &mut Database<Stage>, stage: CompileStage) {
    let rules = fold_constants(stage);
    db.run_pass(stage, rules.required(), |rewriter, cache| {
        RewriteDriver::new(&rules).with_cache(cache).run(rewriter)
    })
    .expect("the pass succeeds");
}

/// `(2 + 3) * 3` is constant; adding the unknown `%x` is not.
const FOLDABLE: &str = "stage @lowered fn @f(i64) -> i64;
specialize @lowered fn @f(i64) -> i64 cfg {
  ^entry(%x: i64) {
    %a = constant 2 -> i64;
    %b = constant 3 -> i64;
    %s = add %a, %b -> i64;
    %t = mul %s, %b -> i64;
    %u = add %t, %x -> i64;
    ret %u;
  }
}
";

/// [`FOLDABLE`] with `%t` folded to 15. The inserted `constant` has no name,
/// so it prints as its SSA id, `%7`.
const FOLDED: &str = "stage @lowered fn @f(i64) -> i64;
specialize @lowered fn @f(i64) -> i64 cfg {
  ^entry(%x: i64) {
    %7 = constant 15 -> i64;
    %u = add %7, %x -> i64;
    ret %u;
  }
}
";

#[test]
fn a_pass_folds_what_constprop_found_constant() {
    let (mut db, stage) = database(FOLDABLE);
    fold(&mut db, stage);
    assert_eq!(db.program().sprint(), printed(FOLDED));
}

#[test]
fn the_pass_after_a_fold_gets_fresh_facts() {
    let (mut db, stage) = database(FOLDABLE);
    NUM_RUNS.set(0);
    fold(&mut db, stage);
    assert_eq!((NUM_RUNS.get(), db.revision()), (1, 1));
    // The fold edited the IR, so the second pass computes constprop again,
    // finds nothing left to fold, and leaves the revision as it is.
    fold(&mut db, stage);
    assert_eq!((NUM_RUNS.get(), db.revision()), (2, 1));
    assert_eq!(db.program().sprint(), printed(FOLDED));
}

/// `main` calls `helper(3)`, so from `main` alone `%z` is 4.
const CALLS_HELPER: &str = "stage @lowered fn @main() -> i64;
stage @lowered fn @helper(i64) -> i64;
specialize @lowered fn @main() -> i64 cfg {
  ^entry() {
    %three = constant 3 -> i64;
    %r = call.named @helper(%three) -> i64;
    ret %r;
  }
}
specialize @lowered fn @helper(i64) -> i64 cfg {
  ^entry(%y: i64) {
    %one = constant 1 -> i64;
    %z = add %y, %one -> i64;
    ret %z;
  }
}
";

#[test]
fn a_function_other_code_may_call_is_not_folded_for_one_caller() {
    let (mut db, stage) = database(CALLS_HELPER);

    // Rooted at `main` only, `helper`'s `%z` would be 4.
    let program = db.program();
    let main = Callee::Function(program.lookup_function_by_name("main").unwrap());
    let engine: ToyConstProp<'_, i64> = ToyConstProp::new(program).with_linker(CrossStageLinker);
    let from_main = engine.analyze(stage, main, []).unwrap();
    let z = helper_result(program, stage);
    assert_eq!(
        from_main.joined_fact_at(stage, z),
        Some(ConstPropValue::Const(4))
    );

    // The stage-wide analysis also roots `helper` with an unknown argument,
    // so `%z` is unknown and the pass leaves it.
    let before = db.program().sprint();
    fold(&mut db, stage);
    assert_eq!(db.program().sprint(), before);
}
