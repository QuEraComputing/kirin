//! A small tail of folds that exposes its next match behind the walk cursor.
use super::common::BenchmarkLanguage as L;
use kirin::prelude::*;
use kirin_arith::Arith;
use kirin_ir::{SSAKind, Use, walk::WalkResult};
use kirin_rewrite::{PatternRule, RuleSet, StatementRule};

/// Looks up to `M` (the match cost) left-operand producers of an addition for
/// `(a - b) + b`. The prefix has no subtractions, so it never matches.
struct SearchAdd(usize);

impl StatementRule<L> for SearchAdd {
    fn apply(
        &mut self,
        rw: &mut Rewriter<'_, L>,
        stmt: Statement,
    ) -> Result<WalkResult, RewriteError> {
        let mut op = stmt.expect_info(rw.stage()).definition();
        for _ in 0..self.0 {
            let L::Arith(Arith::Add { lhs, rhs, result }) = *op else {
                break;
            };
            let SSAKind::Result(parent, _) = *lhs.expect_info(rw.stage()).kind() else {
                break;
            };
            op = parent.expect_info(rw.stage()).definition();
            if let L::Arith(Arith::Sub { lhs: x, rhs: y, .. }) = *op
                && y == rhs
            {
                rw.replace_all_uses(result, x)?;
                break;
            }
        }
        Ok(WalkResult::Advance)
    }
}

struct FoldReturnedPair;

impl StatementRule<L> for FoldReturnedPair {
    fn apply(
        &mut self,
        rw: &mut Rewriter<'_, L>,
        stmt: Statement,
    ) -> Result<WalkResult, RewriteError> {
        let stage = rw.stage();
        let L::Arith(Arith::Neg { operand, result }) = *stmt.expect_info(stage).definition() else {
            return Ok(WalkResult::Advance);
        };
        let returned = SSAValue::from(result).expect_info(stage).uses().iter().any(|site| {
            matches!(*site, Use::StatementOperand {stmt, ..} if matches!(stmt.expect_info(stage).definition(), L::Return(_)))
        });
        if returned
            && let SSAKind::Result(parent, _) = *operand.expect_info(stage).kind()
            && let L::Arith(Arith::Neg { operand: x, .. }) = *parent.expect_info(stage).definition()
        {
            rw.replace_all_uses(result, x)?;
        }
        Ok(WalkResult::Advance)
    }
}

pub(super) fn rules(match_cost: usize) -> RuleSet<L> {
    let mut rules = RuleSet::build(|_| Vec::<PatternRule<L>>::new()).unwrap();
    rules.add_statement_rule("search_add", move || SearchAdd(match_cost));
    rules.add_statement_rule("fold_returned_pair", || FoldReturnedPair);
    rules
}
