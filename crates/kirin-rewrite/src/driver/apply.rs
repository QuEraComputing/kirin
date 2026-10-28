use kirin_ir::walk::WalkResult;
use kirin_ir::{Dialect, GetInfo, RewriteError, Rewriter, SSAValue, Statement};
use kirin_wildcard::{PatternLanguage, PatternStatement};

use super::{DriverError, Match};
use crate::{NativeValue, RewriteRule, RuleSet};

impl Match {
    /// Run `rule`'s rewrite body, in order, for this match rooted at `root`,
    /// and return how its walk result says the driver goes on walking.
    pub(crate) fn apply<L: Dialect>(
        mut self,
        rules: &RuleSet<L>,
        rule: &RewriteRule,
        rewriter: &mut Rewriter<'_, L>,
        root: Statement,
    ) -> Result<WalkResult, DriverError> {
        let patterns = rules.patterns();
        let failed = |error: RewriteError| DriverError::Rewrite {
            rule: rule.name().to_string(),
            stmt: root,
            error,
        };
        for &action in rule.actions() {
            match action.expect_info(patterns).definition() {
                PatternLanguage::Wildcard(PatternStatement::Replace(replace)) => {
                    let matched = self.ssa(rule, replace.matched())?;
                    let replacement = self.ssa(rule, replace.replacement())?;
                    rewriter
                        .replace_all_uses(matched, replacement)
                        .map_err(failed)?;
                }
                PatternLanguage::Wildcard(PatternStatement::Erase(erase)) => {
                    let target = self
                        .statement(rule.body()[erase.target()])
                        .expect("RewriteRule::new checked `erase` names a matched op");
                    rewriter.erase_statement(target).map_err(failed)?;
                }
                PatternLanguage::Wildcard(PatternStatement::NativeCall(call)) => {
                    let args = call
                        .args()
                        .iter()
                        .map(|&arg| self.value(rule, arg))
                        .collect::<Result<Vec<_>, _>>()?;
                    let function = rules
                        .natives()
                        .get_action(call.name())
                        .expect("RuleSet::add checked the function is registered");
                    let found = function(rewriter, &args).map_err(failed)?;
                    self.bind_results(rule, patterns, action, call.name(), found)?;
                }
                _ => unreachable!("RewriteRule::new rejects other rewrite-body statements"),
            }
        }
        let Some(walk_result) = rule.walk_result() else {
            return Ok(WalkResult::Advance);
        };
        match walk_result.expect_info(patterns).definition() {
            PatternLanguage::Wildcard(PatternStatement::Advance(_)) => Ok(WalkResult::Advance),
            PatternLanguage::Wildcard(PatternStatement::Skip(_)) => Ok(WalkResult::Skip),
            PatternLanguage::Wildcard(PatternStatement::Interrupt(_)) => Ok(WalkResult::Interrupt),
            PatternLanguage::Wildcard(PatternStatement::WalkResultOf(walk)) => {
                match self.value(rule, walk.value())? {
                    NativeValue::WalkResult(result) => Ok(result),
                    NativeValue::Bool(_) | NativeValue::Value(_) => {
                        Err(DriverError::NotAWalkResult {
                            rule: rule.name().to_string(),
                        })
                    }
                }
            }
            _ => unreachable!("RewriteRule::new rejects other rewrite-body terminators"),
        }
    }

    /// The SSA value pattern value `value` is bound to.
    fn ssa(&self, rule: &RewriteRule, value: SSAValue) -> Result<SSAValue, DriverError> {
        match self.value(rule, value)? {
            NativeValue::Value(value) => Ok(value),
            NativeValue::Bool(_) | NativeValue::WalkResult(_) => Err(DriverError::NotAnSsaValue {
                rule: rule.name().to_string(),
            }),
        }
    }
}
