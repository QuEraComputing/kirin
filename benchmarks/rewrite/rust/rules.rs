//! The timed edits: rule sets for the driver, and the raw `Rewriter` edits.
use std::marker::PhantomData;

use kirin::prelude::*;
use kirin_arith::Arith;
use kirin_constant::Constant;
use kirin_ir::SSAKind;
use kirin_ir::walk::WalkResult;
use kirin_rewrite::{RewriteSummary, RuleSet, StatementRule};
use kirin_test_languages::SimpleType;
use kirin_wildcard::rewrite_rule;
use serde_json::{Value, json};

use super::common::BenchmarkLanguage as L;
use super::common::language::Int;

/// `-(-x)` is `x`. The two `neg`s are left for the driver's cleanup.
#[rewrite_rule(language = L)]
fn double_neg() {
    let a = op![Arith::neg(x)];
    let r = op![Arith::neg(a)];
    rewrite!(r, {
        replace!(r, x);
    })
}

/// `double_neg` written by hand: the same match and the same edit.
struct DoubleNeg;

impl StatementRule<L> for DoubleNeg {
    fn apply(
        &mut self,
        rewriter: &mut Rewriter<'_, L>,
        stmt: Statement,
    ) -> Result<WalkResult, RewriteError> {
        let stage = rewriter.stage();
        let L::Arith(Arith::Neg { operand, result }) = stmt.expect_info(stage).definition() else {
            return Ok(WalkResult::Advance);
        };
        let SSAKind::Result(producer, _) = *operand.expect_info(stage).kind() else {
            return Ok(WalkResult::Advance);
        };
        let L::Arith(Arith::Neg { operand: x, .. }) = producer.expect_info(stage).definition()
        else {
            return Ok(WalkResult::Advance);
        };
        let (r, x) = (SSAValue::from(*result), *x);
        rewriter.replace_all_uses(r, x)?;
        Ok(WalkResult::Advance)
    }
}

/// The rule set a driver variant runs; building it is outside the timer.
pub fn rule_set(variant: &str) -> RuleSet<L> {
    match variant {
        "pattern" => RuleSet::build(|stage| vec![double_neg(stage)]).expect("the rule is valid"),
        "native" => {
            let mut rules = empty();
            rules.add_statement_rule("double_neg", || DoubleNeg);
            rules
        }
        "unroll" => {
            let mut rules = empty();
            rules.add_statement_rule("unroll_sub", || super::unroll::UnrollSub);
            rules
        }
        "cleanup" => empty(),
        _ => panic!("unknown driver variant {variant}"),
    }
}

fn empty() -> RuleSet<L> {
    RuleSet::build(|_| Vec::<kirin_rewrite::PatternRule<L>>::new()).expect("valid")
}

pub fn summary(summary: RewriteSummary) -> Value {
    json!({"sweeps": summary.sweeps, "applied": summary.applied, "erased": summary.erased})
}

/// The body of `@benchmark`, the stage's only function.
pub fn body(stage: &StageInfo<L>) -> CFG {
    let function = stage
        .staged_function_arena()
        .iter()
        .flat_map(|function| function.specializations())
        .map(|specialization| *specialization.definition())
        .next()
        .expect("one function");
    let L::Function { body, .. } = function.expect_info(stage).definition() else {
        panic!("not a function");
    };
    *body
}

/// The non-terminator statements of `@benchmark`'s entry block, in order,
/// and its terminator.
fn entry_statements(stage: &StageInfo<L>) -> (Vec<Statement>, Statement) {
    let entry = body(stage).blocks(stage).next().expect("an entry block");
    let terminator = entry.terminator(stage).expect("a terminator");
    (entry.statements(stage).collect(), terminator)
}

/// Run the raw edit `op` once per statement of the entry block; return the
/// number of edits.
pub fn primitive(op: &str, rewriter: &mut Rewriter<'_, L>) -> Result<usize, RewriteError> {
    let (statements, terminator) = entry_statements(rewriter.stage());
    for &stmt in &statements {
        match op {
            // Redirect `a + b`'s uses to `a`, then erase it.
            "bypass" => {
                let L::Arith(Arith::Add { lhs, result, .. }) =
                    stmt.expect_info(rewriter.stage()).definition()
                else {
                    panic!("bypass expects additions");
                };
                let (r, lhs) = (SSAValue::from(*result), *lhs);
                rewriter.replace_all_uses(r, lhs)?;
                rewriter.erase_statement(stmt)?;
            }
            // Create an unused `constant 0` before it.
            "insert" => {
                let result = rewriter.new_result(SimpleType::I64);
                let constant = L::Constant(Constant {
                    value: Int(0),
                    result,
                    marker: PhantomData,
                });
                rewriter.create(InsertionPoint::Before(stmt), constant)?;
            }
            // Move it to the end of the block: in order, this rebuilds the
            // same block.
            "move" => rewriter.move_before(stmt, terminator)?,
            "erase" => rewriter.erase_statement(stmt)?,
            _ => panic!("unknown primitive {op}"),
        }
    }
    Ok(statements.len())
}
