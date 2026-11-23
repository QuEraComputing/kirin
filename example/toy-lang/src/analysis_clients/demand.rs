use std::marker::PhantomData;
use std::sync::Arc;

use kirin::prelude::*;
use kirin_arith::{ArithType, ArithValue};
use kirin_constant::Constant;
use kirin_demand::{Demand, DemandAnalysis, DemandCallCtx};
use kirin_interpreter::{AnalysisResults, Callee, InterpreterError, LinkTarget, StrongDemand};
use kirin_ir::walk::WalkResult;
use kirin_rewrite::{
    Analysis, AnalysisCache, AnalysisError, AnalysisKind, Database, PatternRule, RewriteDriver,
    RuleSet, StatementRule,
};

use super::common::{helper_result, new_database, printed, roots};
use crate::language::LowLevel;
use crate::stage::Stage;

/// One function's demand result.
type DemandResults = AnalysisResults<(LinkTarget, DemandCallCtx), SSAValue, Demand, StrongDemand>;

/// Demand over a whole stage, rooting every function with all its outputs
/// demanded, so every function counts as an entry point.
struct DemandFacts;

/// The demand result of each function of a stage.
struct StageDemand(Vec<DemandResults>);

impl StageDemand {
    /// Whether any function's result demands `value` at `stage`. `Dead` is the
    /// lattice's bottom and is not stored, so a value the analysis saw but
    /// nothing demands has no fact.
    fn is_demanded(&self, stage: CompileStage, value: SSAValue) -> bool {
        self.0
            .iter()
            .any(|results| results.joined_fact_at(stage, value) == Some(Demand::Live))
    }
}

impl AnalysisKind for DemandFacts {
    type Key = CompileStage;
    type Output = StageDemand;
}

impl Analysis<Stage> for DemandFacts {
    fn run(db: &Database<Stage>, stage: &CompileStage) -> Result<StageDemand, AnalysisError> {
        let program = db.program();
        let roots = match program.stage(*stage).ok_or(AnalysisError::UnknownStage)? {
            Stage::Source(info) => roots(info),
            Stage::Lowered(info) => roots(info),
        };
        let mut results = Vec::new();
        for (specialization, _) in roots {
            // Toy functions return one value.
            let analyzed = DemandAnalysis::<Stage, InterpreterError>::new(program)
                .analyze(*stage, Callee::Specialized(specialization), [Demand::Live])
                .map_err(AnalysisError::failed::<Self>)?;
            results.push(analyzed);
        }
        Ok(StageDemand(results))
    }
}

/// Moves the uses of an `arith` result that nothing demands onto a new
/// `constant 0`, so the result has no uses and the driver's dead-code cleanup
/// erases the statement. No use of the result is ever read, so the value
/// passed in its place does not matter.
///
/// Every `arith` statement existed when [`DemandFacts`] ran, since this pass
/// creates none, so a missing fact means not demanded.
struct FreeUndemanded {
    stage: CompileStage,
    demand: Arc<StageDemand>,
}

impl StatementRule<LowLevel> for FreeUndemanded {
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
        let has_uses = !result.expect_info(rewriter.stage()).uses().is_empty();
        if !has_uses || self.demand.is_demanded(self.stage, result) {
            return Ok(WalkResult::Advance);
        }
        let placeholder = rewriter.new_result(ArithType::I64);
        rewriter.insert_before(
            stmt,
            LowLevel::Constant(Constant {
                value: ArithValue::I64(0),
                result: placeholder,
                marker: PhantomData,
            }),
        )?;
        rewriter.replace_all_uses(result, SSAValue::from(placeholder))?;
        Ok(WalkResult::Advance)
    }
}

/// A rule set with `free_undemanded`, which reads [`DemandFacts`] for
/// `stage`.
fn free_undemanded(stage: CompileStage) -> RuleSet<LowLevel> {
    let mut rules = RuleSet::build(|_| Vec::<PatternRule<LowLevel>>::new())
        .expect("an empty rule set is valid");
    rules
        .add_statement_rule("free_undemanded", move |cache: &AnalysisCache| {
            FreeUndemanded {
                stage,
                demand: cache.get::<DemandFacts>(&stage).expect("declared"),
            }
        })
        .requires::<DemandFacts>(stage);
    rules
}

/// Run `free_undemanded` over `stage` as one pass.
fn free(db: &mut Database<Stage>, stage: CompileStage) {
    let rules = free_undemanded(stage);
    db.run_pass(stage, rules.required(), |rewriter, cache| {
        RewriteDriver::new(&rules).with_cache(cache).run(rewriter)
    })
    .expect("the pass succeeds");
}

fn demand_database(input: &str) -> (Database<Stage>, CompileStage) {
    let (mut db, stage) = new_database(input);
    db.register::<DemandFacts>();
    (db, stage)
}

/// `@first` never reads `%q`, so `%x` is not demanded, though the call uses
/// it.
const DEAD_ARGUMENT: &str = "stage @lowered fn @main(i64) -> i64;
stage @lowered fn @first(i64, i64) -> i64;
specialize @lowered fn @main(i64) -> i64 cfg {
  ^entry(%a: i64) {
    %x = mul %a, %a -> i64;
    %r = call.named @first(%a, %x) -> i64;
    ret %r;
  }
}
specialize @lowered fn @first(i64, i64) -> i64 cfg {
  ^entry(%p: i64, %q: i64) {
    ret %p;
  }
}
";

/// [`DEAD_ARGUMENT`] with the call reading a placeholder and the `mul`
/// erased. The placeholder has no name, so it prints as its SSA id, `%5`.
const DEAD_ARGUMENT_FREED: &str = "stage @lowered fn @main(i64) -> i64;
stage @lowered fn @first(i64, i64) -> i64;
specialize @lowered fn @main(i64) -> i64 cfg {
  ^entry(%a: i64) {
    %5 = constant 0 -> i64;
    %r = call.named @first(%a, %5) -> i64;
    ret %r;
  }
}
specialize @lowered fn @first(i64, i64) -> i64 cfg {
  ^entry(%p: i64, %q: i64) {
    ret %p;
  }
}
";

#[test]
fn an_undemanded_value_with_a_use_is_freed_and_erased() {
    let (mut db, stage) = demand_database(DEAD_ARGUMENT);
    free(&mut db, stage);
    assert_eq!(db.program().sprint(), printed(DEAD_ARGUMENT_FREED));
}

/// `main` ignores what `helper` returns.
const IGNORES_HELPER: &str = "stage @lowered fn @main(i64) -> i64;
stage @lowered fn @helper(i64) -> i64;
specialize @lowered fn @main(i64) -> i64 cfg {
  ^entry(%a: i64) {
    %r = call.named @helper(%a) -> i64;
    ret %a;
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
fn a_function_other_code_may_call_keeps_what_it_returns() {
    let (mut db, stage) = demand_database(IGNORES_HELPER);

    // Rooted at `main` only, nothing demands `helper`'s `%z`.
    let program = db.program();
    let main = Callee::Function(program.lookup_function_by_name("main").unwrap());
    let from_main = DemandAnalysis::<Stage, InterpreterError>::new(program)
        .analyze(stage, main, [Demand::Live])
        .unwrap();
    let z = helper_result(program, stage);
    assert_ne!(from_main.joined_fact_at(stage, z), Some(Demand::Live));

    // The stage-wide analysis also roots `helper` with its output demanded,
    // so `%z` is demanded and the pass leaves it.
    let before = db.program().sprint();
    free(&mut db, stage);
    assert_eq!(db.program().sprint(), before);
}

#[test]
fn a_demanded_value_is_left_alone() {
    let input = "stage @lowered fn @f(i64) -> i64;
specialize @lowered fn @f(i64) -> i64 cfg {
  ^entry(%x: i64) {
    %y = add %x, %x -> i64;
    ret %y;
  }
}
";
    let (mut db, stage) = demand_database(input);
    free(&mut db, stage);
    assert_eq!(db.program().sprint(), printed(input));
}
