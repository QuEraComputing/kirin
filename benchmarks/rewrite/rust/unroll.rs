use super::common::{BenchmarkLanguage as L, language::Int};
use kirin::prelude::*;
use kirin_arith::Arith;
use kirin_constant::Constant;
use kirin_ir::{SSAKind, walk::WalkResult};
use kirin_rewrite::StatementRule;
use kirin_test_languages::SimpleType;
use std::marker::PhantomData;

/// `v - k` with a constant `k > 1` is `(v - (k - 1)) - 1`. The new
/// `v - (k - 1)` is created before the matched statement, so it matches again
/// until `k` is 1: a cascade of `k - 1` rewrites per `sub`. As in Python, an
/// unused match is not rewritten.
pub(super) struct UnrollSub;

impl StatementRule<L> for UnrollSub {
    fn apply(
        &mut self,
        rewriter: &mut Rewriter<'_, L>,
        stmt: Statement,
    ) -> Result<WalkResult, RewriteError> {
        let stage = rewriter.stage();
        let L::Arith(Arith::Sub { lhs, rhs, result }) = stmt.expect_info(stage).definition() else {
            return Ok(WalkResult::Advance);
        };
        let r = SSAValue::from(*result);
        if r.expect_info(stage).uses().is_empty() {
            return Ok(WalkResult::Advance);
        }
        let SSAKind::Result(producer, _) = *rhs.expect_info(stage).kind() else {
            return Ok(WalkResult::Advance);
        };
        let L::Constant(Constant { value: Int(k), .. }) = producer.expect_info(stage).definition()
        else {
            return Ok(WalkResult::Advance);
        };
        let (lhs, k) = (*lhs, *k);
        if k <= 1 {
            return Ok(WalkResult::Advance);
        }
        let at = InsertionPoint::Before(stmt);
        let rest = emit(rewriter, at, |result| {
            L::Constant(Constant {
                value: Int(k - 1),
                result,
                marker: PhantomData,
            })
        })?;
        let partial = emit(rewriter, at, |result| {
            L::Arith(Arith::Sub {
                lhs,
                rhs: rest,
                result,
            })
        })?;
        let one = emit(rewriter, at, |result| {
            L::Constant(Constant {
                value: Int(1),
                result,
                marker: PhantomData,
            })
        })?;
        let unrolled = emit(rewriter, at, |result| {
            L::Arith(Arith::Sub {
                lhs: partial,
                rhs: one,
                result,
            })
        })?;
        rewriter.replace_all_uses(r, unrolled)?;
        Ok(WalkResult::Advance)
    }
}

/// Create `definition(result)` at `at` with a fresh `i64` result.
fn emit(
    rewriter: &mut Rewriter<'_, L>,
    at: InsertionPoint,
    definition: impl FnOnce(ResultValue) -> L,
) -> Result<SSAValue, RewriteError> {
    let result = rewriter.new_result(SimpleType::I64);
    rewriter.create(at, definition(result))?;
    Ok(result.into())
}
