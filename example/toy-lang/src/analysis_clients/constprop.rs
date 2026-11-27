use std::collections::hash_map::Entry;
use std::marker::PhantomData;
use std::sync::Arc;

use kirin::ir::walk::WalkResult;
use kirin::prelude::*;
use kirin_arith::{ArithType, ArithValue};
use kirin_constant::Constant;
use kirin_constprop::{CallCtx, ConstPropValue};
use kirin_interpreter::{AnalysisResults, Callee, CrossStageLinker, ForwardEval, LinkTarget};
use kirin_rewrite::{
    Analysis, AnalysisCache, AnalysisError, AnalysisKind, Database, PatternRule, RuleSet,
    StatementRule,
};
use rustc_hash::FxHashMap;

use super::common::roots;
use super::log;
use crate::interpreter::ToyConstProp;
use crate::language::LowLevel;
use crate::stage::Stage;

type Value = ConstPropValue<i64>;
/// One function's constprop result.
type Results = AnalysisResults<(LinkTarget, CallCtx<i64>), SSAValue, Value, ForwardEval>;

/// Constprop over a whole stage, rooting every function with unknown
/// arguments, so every function counts as an entry point.
pub struct ConstPropFacts;

impl ConstPropFacts {
    pub const NAME: &str = "constprop";
}

/// Each value's constprop fact at a stage, joined over every function's
/// result when the analysis finishes, so a read is one lookup.
pub struct StageConstants(FxHashMap<SSAValue, Value>);

impl StageConstants {
    /// Join the facts of `results`' contexts at `stage` into `self`.
    fn add(&mut self, stage: CompileStage, results: &Results) {
        for context in results.contexts() {
            // Calls may cross into another stage, whose SSA ids say nothing
            // about this one's.
            if results
                .callable_of(context)
                .is_none_or(|target| target.stage != stage)
            {
                continue;
            }
            // A value reached from several roots or contexts gets the join of
            // its facts.
            for (value, fact) in results.facts(context) {
                match self.0.entry(value) {
                    Entry::Occupied(mut entry) => {
                        let joined = entry.get().join(fact);
                        entry.insert(joined);
                    }
                    Entry::Vacant(entry) => {
                        entry.insert(fact.clone());
                    }
                }
            }
        }
    }

    /// `value`'s fact joined over every function's result. `None` means no
    /// result reached `value`.
    pub fn value_at(&self, value: SSAValue) -> Option<Value> {
        self.0.get(&value).cloned()
    }
}

impl AnalysisKind for ConstPropFacts {
    type Key = CompileStage;
    type Output = StageConstants;
}

impl Analysis<Stage> for ConstPropFacts {
    fn run(db: &Database<Stage>, stage: &CompileStage) -> Result<StageConstants, AnalysisError> {
        StageConstants::compute(db.program(), *stage)
    }
}

impl StageConstants {
    /// Analyze every function of `stage` as a root and join the facts.
    pub fn compute(program: &Program<Stage>, stage: CompileStage) -> Result<Self, AnalysisError> {
        let roots = match program.stage(stage).ok_or(AnalysisError::UnknownStage)? {
            Stage::Source(info) => roots(info),
            Stage::Lowered(info) => roots(info),
        };
        let mut constants = StageConstants(FxHashMap::default());
        let mut run = log::Run::start(ConstPropFacts::NAME);
        for (specialization, arity) in roots {
            let root = run.root(specialization);
            let engine: ToyConstProp<'_, i64> =
                ToyConstProp::new(program).with_linker(CrossStageLinker);
            let analyzed = engine
                .analyze(
                    stage,
                    Callee::Specialized(specialization),
                    (0..arity).map(|_| Value::Top),
                )
                .map_err(AnalysisError::failed::<ConstPropFacts>)?;
            constants.add(stage, &analyzed);
            run.end_root(root);
        }
        run.finish();
        Ok(constants)
    }
}

/// Replaces the result of an `arith` statement that constprop found constant
/// with a new `constant` inserted before it. The driver's dead-code cleanup
/// then erases the statement.
pub struct FoldConstants {
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
        let Some(ConstPropValue::Const(c)) = log::lookup(|| self.facts.value_at(result)) else {
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
pub fn fold_constants(stage: CompileStage) -> RuleSet<LowLevel> {
    let mut rules = RuleSet::build(|_| Vec::<PatternRule<LowLevel>>::new())
        .expect("an empty rule set is valid");
    rules
        .add_statement_rule("fold_constants", move |cache: &AnalysisCache| {
            FoldConstants {
                facts: cache.get::<ConstPropFacts>(&stage).expect("declared"),
            }
        })
        .requires::<ConstPropFacts>(stage);
    rules
}

/// A rule set with `fold_constants` reading `facts`, computed by the caller
/// rather than the database.
pub fn fold_constants_with(facts: Arc<StageConstants>) -> RuleSet<LowLevel> {
    let mut rules = RuleSet::build(|_| Vec::<PatternRule<LowLevel>>::new())
        .expect("an empty rule set is valid");
    rules.add_statement_rule("fold_constants", move |_: &AnalysisCache| FoldConstants {
        facts: Arc::clone(&facts),
    });
    rules
}

#[cfg(test)]
mod tests {
    use kirin::prelude::*;
    use kirin_constprop::ConstPropValue;
    use kirin_interpreter::{Callee, CrossStageLinker};
    use kirin_rewrite::{Database, RewriteDriver};

    use super::super::common::{helper_result, new_database, printed};
    use super::super::log;
    use super::{ConstPropFacts, fold_constants};
    use crate::interpreter::ToyConstProp;
    use crate::stage::Stage;

    /// How many times [`ConstPropFacts`] has run on this thread since the last
    /// call.
    fn runs() -> usize {
        log::take()
            .iter()
            .filter(|run| run.analysis == ConstPropFacts::NAME)
            .count()
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
        runs();
        fold(&mut db, stage);
        assert_eq!((runs(), db.revision()), (1, 1));
        // The fold edited the IR, so the second pass computes constprop again,
        // finds nothing left to fold, and leaves the revision as it is.
        fold(&mut db, stage);
        assert_eq!((runs(), db.revision()), (1, 1));
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
        let engine: ToyConstProp<'_, i64> =
            ToyConstProp::new(program).with_linker(CrossStageLinker);
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
}
