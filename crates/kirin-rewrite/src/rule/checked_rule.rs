use std::collections::HashSet;

use kirin_ir::{Dialect, GetInfo, SSAKind, SSAValue, StageInfo, Statement};
use kirin_wildcard::{
    ActionFn, CheckFn, NativeCall, PatternLanguage, PatternRule, PatternStatement, Rewrite,
};

use super::{RuleError, RuleErrorKind};

/// A `wildcard.pattern`, checked once so the driver can run it without
/// re-validating.
///
/// The pattern stays the source of truth: every field is a statement of the
/// [`RuleSet`](crate::RuleSet)'s pattern stage, and the rule only records
/// where the parts of the pattern are. Each `native_call` is resolved to
/// its one function here, so the driver never looks a function up.
pub struct CheckedRule<L: Dialect> {
    name: String,
    /// The op the match is rooted at.
    root: Statement,
    /// The `native_call`s and `constraint`s of the pattern body, in order.
    checks: Vec<Step<CheckFn<L>>>,
    /// The rewrite body, in order.
    actions: Vec<Step<ActionFn<L>>>,
    /// Whether the rewrite body erases the root.
    erases_root: bool,
    /// The rewrite body's terminator, saying how the driver goes on walking.
    /// `None` advances.
    walk_result: Option<Statement>,
}

/// A statement of a pattern or rewrite body, in the order the driver runs
/// them.
pub(crate) enum Step<F> {
    /// A `native_call`, with the function it calls.
    Native { stmt: Statement, function: F },
    /// Any other statement: a `constraint`, `replace` or `erase`.
    Other(Statement),
}

impl<L: Dialect> CheckedRule<L> {
    /// The name the pattern was given.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The pattern op the match is rooted at.
    pub fn root(&self) -> Statement {
        self.root
    }

    pub(crate) fn checks(&self) -> &[Step<CheckFn<L>>] {
        &self.checks
    }

    pub(crate) fn actions(&self) -> &[Step<ActionFn<L>>] {
        &self.actions
    }

    pub(crate) fn walk_result(&self) -> Option<Statement> {
        self.walk_result
    }

    /// Whether the rule erases its root, the statement the driver is
    /// visiting when the rule runs. Only some walk orders allow that, so the
    /// driver checks it against the order of each run.
    pub fn erases_root(&self) -> bool {
        self.erases_root
    }

    /// Check `rule`'s pattern and record its parts.
    ///
    /// The matched ops are the root and, through their operands, the ops
    /// producing them, stopping at holes. Every other op is rejected, as is
    /// anything the driver cannot run yet.
    ///
    /// A rule may erase its root, which is recorded so the driver can check
    /// it against the walk order (see [`kirin_ir::walk`]). The other ops a
    /// rule may erase are producers of the root. Matched ops own no bodies,
    /// so a producer is never an ancestor of the root, and no other erase can
    /// break the walk in any order.
    pub(crate) fn new(
        stage: &StageInfo<PatternLanguage<L>>,
        natives: PatternRule<L>,
    ) -> Result<Self, RuleError> {
        let pattern = natives.pattern();
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

        let rewrite = pattern
            .rewrite(stage)
            .ok_or_else(|| RuleError::new(&name, RuleErrorKind::MissingRewrite))?;
        let body: Vec<Statement> = pattern.body().statements(stage).collect();
        let root = rewrite.root();
        if !body.contains(&root) {
            return Err(RuleError::new(&name, RuleErrorKind::RootNotInBody));
        }

        let matched = Self::matched_ops(stage, &name, &body, root)?;
        let checks = Self::build_checks(stage, &natives, &name, &body, &matched)?;
        let (actions, erases_root) =
            Self::build_actions(stage, &natives, &name, rewrite, &matched, root)?;
        let walk_result = Self::check_walk_result(stage, &name, rewrite, &actions)?;

        Ok(Self {
            name,
            root,
            checks,
            actions,
            erases_root,
            walk_result,
        })
    }

    /// The ops the match covers: `root` and, through their operands, the ops
    /// producing them, stopping at holes. Rejects an op that owns bodies or
    /// has successors, and an operand that is neither a hole nor an op.
    fn matched_ops(
        stage: &StageInfo<PatternLanguage<L>>,
        name: &str,
        body: &[Statement],
        root: Statement,
    ) -> Result<HashSet<Statement>, RuleError> {
        let error = |kind| RuleError::new(name, kind);
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
                    position: position(body, stmt),
                }));
            }
            for &operand in op.arguments() {
                match producer(stage, operand).map(|p| (p, p.expect_info(stage).definition())) {
                    Some((_, PatternLanguage::Wildcard(PatternStatement::Hole(_)))) => {}
                    Some((producer, PatternLanguage::Op(_))) => pending.push(producer),
                    _ => {
                        return Err(error(RuleErrorKind::UnsupportedOperand {
                            position: position(body, stmt),
                        }));
                    }
                }
            }
        }
        Ok(matched)
    }

    /// The `native_call`s and `constraint`s of the pattern body `body`, in
    /// order, each `native_call` with its one check function. Rejects an op
    /// the match does not cover, a `constraint` not testing a `native_call`'s
    /// result, and any other statement but a hole.
    fn build_checks(
        stage: &StageInfo<PatternLanguage<L>>,
        natives: &PatternRule<L>,
        name: &str,
        body: &[Statement],
        matched: &HashSet<Statement>,
    ) -> Result<Vec<Step<CheckFn<L>>>, RuleError> {
        let error = |kind| RuleError::new(name, kind);
        let mut checks = Vec::new();
        for (position, &stmt) in body.iter().enumerate() {
            match stmt.expect_info(stage).definition() {
                PatternLanguage::Op(_) if matched.contains(&stmt) => {}
                PatternLanguage::Op(_) => {
                    return Err(error(RuleErrorKind::UnmatchedOp { position }));
                }
                PatternLanguage::Wildcard(PatternStatement::Hole(_)) => {}
                PatternLanguage::Wildcard(PatternStatement::NativeCall(call)) => {
                    // The call needs exactly one function: none or two
                    // reject the rule.
                    let name = || call.name().to_string();
                    let mut found = natives.checks_of(stmt);
                    let function = match (found.next(), found.next()) {
                        (Some(function), None) => function.clone(),
                        (None, _) => return Err(error(RuleErrorKind::UnknownCheck(name()))),
                        (Some(_), Some(_)) => return Err(error(RuleErrorKind::TwoChecks(name()))),
                    };
                    checks.push(Step::Native { stmt, function });
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
                    checks.push(Step::Other(stmt));
                }
                _ => return Err(error(RuleErrorKind::UnexpectedStatement { position })),
            }
        }
        Ok(checks)
    }

    /// The statements of `rewrite`'s body, in order, each `native_call` with
    /// its one action function, and whether one of them erases `root`.
    /// Rejects a `replace` or `erase` of something the match does not cover,
    /// and any other statement.
    #[expect(
        clippy::type_complexity,
        reason = "the steps and `erases_root` are found in one pass over the body"
    )]
    fn build_actions(
        stage: &StageInfo<PatternLanguage<L>>,
        natives: &PatternRule<L>,
        name: &str,
        rewrite: &Rewrite<L::Type>,
        matched: &HashSet<Statement>,
        root: Statement,
    ) -> Result<(Vec<Step<ActionFn<L>>>, bool), RuleError> {
        let error = |kind| RuleError::new(name, kind);
        let mut actions = Vec::new();
        let mut erases_root = false;
        for (position, stmt) in rewrite.body().statements(stage).enumerate() {
            match stmt.expect_info(stage).definition() {
                PatternLanguage::Wildcard(PatternStatement::Replace(replace)) => {
                    let redirects_match =
                        producer(stage, replace.matched()).is_some_and(|p| matched.contains(&p));
                    if !redirects_match {
                        return Err(error(RuleErrorKind::ReplaceNotMatched { position }));
                    }
                    actions.push(Step::Other(stmt));
                }
                PatternLanguage::Wildcard(PatternStatement::Erase(erase)) => {
                    let target = erase.target();
                    erases_root |= target == root;
                    if !matched.contains(&target) {
                        return Err(error(RuleErrorKind::EraseNotMatched { position }));
                    }
                    actions.push(Step::Other(stmt));
                }
                PatternLanguage::Wildcard(PatternStatement::NativeCall(call)) => {
                    // Exactly one function, as for a check.
                    let name = || call.name().to_string();
                    let mut found = natives.actions_of(stmt);
                    let function = match (found.next(), found.next()) {
                        (Some(function), None) => function.clone(),
                        (None, _) => return Err(error(RuleErrorKind::UnknownAction(name()))),
                        (Some(_), Some(_)) => return Err(error(RuleErrorKind::TwoActions(name()))),
                    };
                    actions.push(Step::Native { stmt, function });
                }
                _ => return Err(error(RuleErrorKind::UnsupportedAction { position })),
            }
        }
        Ok((actions, erases_root))
    }

    /// The terminator `rewrite`'s body ends in, if any: a walk result saying
    /// how the driver goes on walking. A `walk_result` must read what a
    /// `native_call` among `actions` computed.
    fn check_walk_result(
        stage: &StageInfo<PatternLanguage<L>>,
        name: &str,
        rewrite: &Rewrite<L::Type>,
        actions: &[Step<ActionFn<L>>],
    ) -> Result<Option<Statement>, RuleError> {
        let error = |kind| RuleError::new(name, kind);
        let walk_result = rewrite.body().terminator(stage);
        if let Some(terminator) = walk_result {
            match terminator.expect_info(stage).definition() {
                PatternLanguage::Wildcard(
                    PatternStatement::Advance(_)
                    | PatternStatement::Skip(_)
                    | PatternStatement::Interrupt(_),
                ) => {}
                PatternLanguage::Wildcard(PatternStatement::WalkResultOf(walk)) => {
                    // Only a `native_call` becomes a `Step::Native`.
                    let computed = producer(stage, walk.value()).is_some_and(|p| {
                        actions
                            .iter()
                            .any(|step| matches!(step, Step::Native { stmt, .. } if *stmt == p))
                    });
                    if !computed {
                        return Err(error(RuleErrorKind::WalkResultWithoutNativeCall));
                    }
                }
                _ => return Err(error(RuleErrorKind::UnexpectedTerminator)),
            }
        }
        Ok(walk_result)
    }
}

/// Where `stmt` is in the pattern body `body`, counting from 0, for errors.
fn position(body: &[Statement], stmt: Statement) -> usize {
    body.iter().position(|&s| s == stmt).unwrap_or_default()
}

/// The `native_call` a [`Step::Native`] runs.
pub(crate) fn native_call<L: Dialect>(
    patterns: &StageInfo<PatternLanguage<L>>,
    stmt: Statement,
) -> &NativeCall<L::Type> {
    match stmt.expect_info(patterns).definition() {
        PatternLanguage::Wildcard(PatternStatement::NativeCall(call)) => call,
        _ => unreachable!("CheckedRule::new makes `Step::Native` only for native_calls"),
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
