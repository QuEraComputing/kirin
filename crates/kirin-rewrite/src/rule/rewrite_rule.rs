use std::collections::HashSet;

use kirin_ir::{Dialect, GetInfo, SSAKind, SSAValue, StageInfo, Statement};
use kirin_wildcard::{PatternLanguage, PatternStatement};

use super::{RuleError, RuleErrorKind};
use crate::NativeRegistry;

/// A `wildcard.pattern`, checked once so the driver can run it without
/// re-validating.
///
/// The pattern stays the source of truth: every field is a statement of the
/// [`RuleSet`](crate::RuleSet)'s pattern stage, and the rule only records
/// where the parts of the pattern are.
#[derive(Clone, Debug)]
pub struct RewriteRule {
    name: String,
    /// The op the match is rooted at.
    root: Statement,
    /// The `native_call`s and `constraint`s of the pattern body, in order.
    checks: Vec<Statement>,
    /// The rewrite body, in order.
    actions: Vec<Statement>,
}

impl RewriteRule {
    /// The name the pattern was given.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The pattern op the match is rooted at.
    pub fn root(&self) -> Statement {
        self.root
    }

    pub(crate) fn checks(&self) -> &[Statement] {
        &self.checks
    }

    pub(crate) fn actions(&self) -> &[Statement] {
        &self.actions
    }

    /// Check `pattern` and record its parts.
    ///
    /// The matched ops are the root and, through their operands, the ops
    /// producing them, stopping at holes. Every other op is rejected, as is
    /// anything the driver cannot run yet.
    ///
    /// A rule must not erase its root: the driver is visiting it, and a walk
    /// forbids erasing the statement being visited (see
    /// [`kirin_ir::walk`]). Dead roots are left to the dead-code cleanup. The
    /// producers a rule may erase are never nested in or around the root,
    /// so no other erase can break the walk.
    pub(crate) fn new<L: Dialect>(
        stage: &StageInfo<PatternLanguage<L>>,
        natives: &NativeRegistry<L>,
        pattern: Statement,
    ) -> Result<Self, RuleError> {
        let not_a_pattern = || RuleError {
            rule: None,
            kind: RuleErrorKind::NotAPattern(pattern),
        };
        let info = pattern.get_info(stage).ok_or_else(not_a_pattern)?;
        let PatternLanguage::Wildcard(PatternStatement::Pattern(pattern)) = info.definition()
        else {
            return Err(not_a_pattern());
        };
        let name = stage
            .symbol_table()
            .resolve(pattern.name())
            .cloned()
            .unwrap_or_default();
        let error = |kind| RuleError::new(&name, kind);

        let rewrite = pattern
            .rewrite(stage)
            .ok_or_else(|| error(RuleErrorKind::MissingRewrite))?;
        let body: Vec<Statement> = pattern.body().statements(stage).collect();
        let root = rewrite.root();
        if !body.contains(&root) {
            return Err(error(RuleErrorKind::RootNotInBody));
        }
        let position = |stmt: Statement| body.iter().position(|&s| s == stmt).unwrap_or_default();

        // The root and its producers.
        let mut matched = HashSet::new();
        let mut pending = vec![root];
        while let Some(stmt) = pending.pop() {
            if !matched.insert(stmt) {
                continue;
            }
            let PatternLanguage::Op(op) = stmt.expect_info(stage).definition() else {
                return Err(error(RuleErrorKind::RootNotAnOp));
            };
            let owns_bodies = op.blocks().next().is_some()
                || op.cfgs().next().is_some()
                || op.digraphs().next().is_some()
                || op.ungraphs().next().is_some()
                || op.successors().next().is_some();
            if owns_bodies {
                return Err(error(RuleErrorKind::OpWithBodies {
                    position: position(stmt),
                }));
            }
            for &operand in op.arguments() {
                match producer(stage, operand).map(|p| (p, p.expect_info(stage).definition())) {
                    Some((_, PatternLanguage::Wildcard(PatternStatement::Hole(_)))) => {}
                    Some((producer, PatternLanguage::Op(_))) => pending.push(producer),
                    _ => {
                        return Err(error(RuleErrorKind::UnsupportedOperand {
                            position: position(stmt),
                        }));
                    }
                }
            }
        }

        let mut checks = Vec::new();
        for (position, &stmt) in body.iter().enumerate() {
            match stmt.expect_info(stage).definition() {
                PatternLanguage::Op(_) if matched.contains(&stmt) => {}
                PatternLanguage::Op(_) => {
                    return Err(error(RuleErrorKind::UnmatchedOp { position }));
                }
                PatternLanguage::Wildcard(PatternStatement::Hole(_)) => {}
                PatternLanguage::Wildcard(PatternStatement::NativeCall(call)) => {
                    if natives.get_check(call.name()).is_none() {
                        return Err(error(RuleErrorKind::UnknownCheck(call.name().to_string())));
                    }
                    checks.push(stmt);
                }
                PatternLanguage::Wildcard(PatternStatement::Constraint(constraint)) => {
                    let computed = producer(stage, constraint.success()).is_some_and(|p| {
                        matches!(
                            p.expect_info(stage).definition(),
                            PatternLanguage::Wildcard(PatternStatement::NativeCall(_))
                        )
                    });
                    if !computed {
                        return Err(error(RuleErrorKind::ConstraintWithoutNativeCall {
                            position,
                        }));
                    }
                    checks.push(stmt);
                }
                _ => return Err(error(RuleErrorKind::UnexpectedStatement { position })),
            }
        }

        let actions: Vec<Statement> = rewrite.body().statements(stage).collect();
        for (position, &stmt) in actions.iter().enumerate() {
            match stmt.expect_info(stage).definition() {
                PatternLanguage::Wildcard(PatternStatement::Replace(replace)) => {
                    let redirects_match =
                        producer(stage, replace.matched()).is_some_and(|p| matched.contains(&p));
                    if !redirects_match {
                        return Err(error(RuleErrorKind::ReplaceNotMatched { position }));
                    }
                }
                PatternLanguage::Wildcard(PatternStatement::Erase(erase)) => {
                    let target = erase.target();
                    if target == root {
                        return Err(error(RuleErrorKind::ErasesRoot));
                    }
                    if !matched.contains(&target) {
                        return Err(error(RuleErrorKind::EraseNotMatched { position }));
                    }
                }
                PatternLanguage::Wildcard(PatternStatement::NativeCall(call)) => {
                    if natives.get_action(call.name()).is_none() {
                        return Err(error(RuleErrorKind::UnknownAction(call.name().to_string())));
                    }
                }
                _ => return Err(error(RuleErrorKind::UnsupportedAction { position })),
            }
        }

        // The body ends in a walk result, if any; a `walk_result` reads what a
        // native call of the body computed.
        if let Some(terminator) = rewrite.body().terminator(stage) {
            match terminator.expect_info(stage).definition() {
                PatternLanguage::Wildcard(
                    PatternStatement::Advance(_)
                    | PatternStatement::Skip(_)
                    | PatternStatement::Interrupt(_),
                ) => {}
                PatternLanguage::Wildcard(PatternStatement::WalkResultOf(walk)) => {
                    let computed = producer(stage, walk.value()).is_some_and(|p| {
                        actions.contains(&p)
                            && matches!(
                                p.expect_info(stage).definition(),
                                PatternLanguage::Wildcard(PatternStatement::NativeCall(_))
                            )
                    });
                    if !computed {
                        return Err(error(RuleErrorKind::WalkResultWithoutNativeCall));
                    }
                }
                _ => return Err(error(RuleErrorKind::UnexpectedTerminator)),
            }
        }

        Ok(Self {
            name,
            root,
            checks,
            actions,
        })
    }
}

/// The statement whose result `value` is, or `None` for a block argument or
/// port.
pub(crate) fn producer<L: Dialect>(stage: &StageInfo<L>, value: SSAValue) -> Option<Statement> {
    match value.get_info(stage)?.kind() {
        SSAKind::Result(stmt, _) => Some(*stmt),
        _ => None,
    }
}
