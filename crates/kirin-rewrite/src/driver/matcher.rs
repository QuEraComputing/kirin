use std::collections::{HashMap, HashSet};

use kirin_ir::{
    Dialect, GetInfo, HasResults, ResultValue, SSAKind, SSAValue, StageInfo, Statement,
};
use kirin_wildcard::{PatternLanguage, PatternStatement};

use super::DriverError;
use crate::rule::{Step, native_call, producer};
use crate::{CheckedRule, NativeValue, RuleSet};

/// A successful match of one rule: what each pattern value and matched
/// pattern statement stands for in the IR.
pub(crate) struct Match {
    values: HashMap<SSAValue, NativeValue>,
    statements: HashMap<Statement, Statement>,
    /// The IR statements in `statements`, so each is matched by at most one
    /// pattern op.
    matched: HashSet<Statement>,
}

impl Match {
    /// Match `rule` rooted at `stmt`, then run its checks. `Ok(None)` if the
    /// structure does not match or a check fails.
    pub(crate) fn find<L: Dialect>(
        rules: &RuleSet<L>,
        rule: &CheckedRule<L>,
        stage: &StageInfo<L>,
        stmt: Statement,
    ) -> Result<Option<Self>, DriverError> {
        let mut found = Self {
            values: HashMap::new(),
            statements: HashMap::new(),
            matched: HashSet::new(),
        };
        if !found.op(rules.patterns(), stage, rule.root(), stmt) {
            return Ok(None);
        }
        found.check(rules, rule, stage, stmt)
    }

    /// What pattern value `value` is bound to.
    pub(crate) fn value<L: Dialect>(
        &self,
        rule: &CheckedRule<L>,
        value: SSAValue,
    ) -> Result<NativeValue, DriverError> {
        self.values
            .get(&value)
            .cloned()
            .ok_or_else(|| DriverError::Unbound {
                rule: rule.name().to_string(),
            })
    }

    /// The IR statement matched pattern statement `stmt` matched.
    pub(crate) fn statement(&self, stmt: Statement) -> Option<Statement> {
        self.statements.get(&stmt).copied()
    }

    /// Bind each result of the pattern `native_call` `call` to `found`.
    pub(crate) fn bind_results<L: Dialect>(
        &mut self,
        rule: &CheckedRule<L>,
        patterns: &StageInfo<PatternLanguage<L>>,
        call: Statement,
        name: &str,
        found: Vec<NativeValue>,
    ) -> Result<(), DriverError> {
        let results: Vec<ResultValue> = call
            .expect_info(patterns)
            .definition()
            .results()
            .copied()
            .collect();
        if results.len() != found.len() {
            return Err(DriverError::NativeArity {
                rule: rule.name().to_string(),
                name: name.to_string(),
                expected: results.len(),
                found: found.len(),
            });
        }
        for (result, value) in results.into_iter().zip(found) {
            self.values.insert(result.into(), value);
        }
        Ok(())
    }

    /// Match pattern op `pattern` against IR statement `stmt`, then its
    /// producers against `stmt`'s.
    ///
    /// Two ops match when they have the same kind and attributes. Stage-local
    /// symbols are compared by name through their respective tables. The
    /// pattern clone's symbol, operand and result IDs are then normalized to
    /// the candidate's before comparing the remaining fields with `==`.
    /// Ops with bodies or successors are rejected when the rule is built.
    fn op<L: Dialect>(
        &mut self,
        patterns: &StageInfo<PatternLanguage<L>>,
        stage: &StageInfo<L>,
        pattern: Statement,
        stmt: Statement,
    ) -> bool {
        // Already matched through another operand, as `?c` is twice in
        // `?r = add ?c, ?c`: it must be the same IR statement again.
        if let Some(&bound) = self.statements.get(&pattern) {
            return bound == stmt;
        }
        // Each IR statement is matched by at most one pattern op: with
        // `?a = neg ?x`, `?b = neg ?y` and `?r = add ?a, ?b`, the IR
        // `%n = neg %x; %s = add %n, %n` does not match, as `?a` and `?b`
        // would both be `%n`'s statement.
        if self.matched.contains(&stmt) {
            return false;
        }
        let Some(info) = stmt.get_info(stage) else {
            return false;
        };
        let concrete = info.definition();
        let PatternLanguage::Op(op) = pattern.expect_info(patterns).definition() else {
            return false;
        };

        // Step 1: an op of another kind, or with another number of operands
        // or results, cannot match, so reject it before cloning the pattern
        // op. Only the outermost enum variant is compared here; ops that pass
        // are compared in full below.
        if std::mem::discriminant(op) != std::mem::discriminant(concrete)
            || op.arguments().count() != concrete.arguments().count()
            || op.results().count() != concrete.results().count()
        {
            return false;
        }

        // Step 2: compare attributes across stages, normalizing local IDs
        // on a clone before using structural equality.
        let mut probe = op.clone();
        {
            let mut pattern_symbols = probe.symbols_mut();
            let mut concrete_symbols = concrete.symbols();
            loop {
                match (pattern_symbols.next(), concrete_symbols.next()) {
                    (Some(pattern_symbol), Some(&concrete_symbol)) => {
                        let Some(name) = patterns.symbol_table().resolve(*pattern_symbol) else {
                            return false;
                        };
                        if stage.symbol_table().resolve(concrete_symbol) != Some(name) {
                            return false;
                        }
                        *pattern_symbol = concrete_symbol;
                    }
                    (None, None) => break,
                    _ => return false,
                }
            }
        }
        for (slot, &operand) in probe.arguments_mut().zip(concrete.arguments()) {
            *slot = operand;
        }
        for (slot, &result) in probe.results_mut().zip(concrete.results()) {
            *slot = result;
        }
        if probe != *concrete {
            return false;
        }

        // Step 3: record what each pattern value stands for in this match,
        // starting with the results.
        self.statements.insert(pattern, stmt);
        self.matched.insert(stmt);
        for (&pattern_result, &result) in op.results().zip(concrete.results()) {
            if !self.bind(pattern_result.into(), result.into()) {
                return false;
            }
        }
        // Step 4: follow each operand. A hole takes whatever value is there;
        // an op must match the IR statement producing that value.
        for (&pattern_operand, &operand) in op.arguments().zip(concrete.arguments()) {
            let Some(producer) = producer(patterns, pattern_operand) else {
                return false;
            };
            match producer.expect_info(patterns).definition() {
                PatternLanguage::Wildcard(PatternStatement::Hole(_)) => {
                    if !self.bind(pattern_operand, operand) {
                        return false;
                    }
                }
                PatternLanguage::Op(_) => {
                    // The operand must be the same result of the IR statement
                    // matching `producer`, which binds it.
                    let (Some(SSAKind::Result(_, want)), Some(SSAKind::Result(defining, index))) = (
                        pattern_operand.get_info(patterns).map(|i| *i.kind()),
                        operand.get_info(stage).map(|i| *i.kind()),
                    ) else {
                        return false;
                    };
                    if want != index || !self.op(patterns, stage, producer, defining) {
                        return false;
                    }
                }
                _ => return false,
            }
        }
        true
    }

    /// Bind pattern value `pattern` to `value`, or check it already is.
    fn bind(&mut self, pattern: SSAValue, value: SSAValue) -> bool {
        self.values
            .entry(pattern)
            .or_insert_with(|| value.into())
            .ssa()
            == Some(value)
    }

    /// Run the pattern body's `native_call`s and `constraint`s, in order,
    /// for the match rooted at `root`.
    fn check<L: Dialect>(
        mut self,
        rules: &RuleSet<L>,
        rule: &CheckedRule<L>,
        stage: &StageInfo<L>,
        root: Statement,
    ) -> Result<Option<Self>, DriverError> {
        let patterns = rules.patterns();
        for step in rule.checks() {
            match step {
                Step::Native { stmt, function } => {
                    let call = native_call(patterns, *stmt);
                    let args = call
                        .args()
                        .iter()
                        .map(|&arg| self.value(rule, arg))
                        .collect::<Result<Vec<_>, _>>()?;
                    let found = function(stage, &args).map_err(|error| {
                        DriverError::native(rule.name(), call.name(), root, error)
                    })?;
                    let Some(found) = found else {
                        return Ok(None);
                    };
                    self.bind_results(rule, patterns, *stmt, call.name(), found)?;
                }
                Step::Other(stmt) => {
                    let PatternLanguage::Wildcard(PatternStatement::Constraint(constraint)) =
                        stmt.expect_info(patterns).definition()
                    else {
                        unreachable!("CheckedRule::new keeps only calls and constraints as checks")
                    };
                    match self.value(rule, constraint.success())?.get::<bool>() {
                        Some(true) => {}
                        Some(false) => return Ok(None),
                        None => {
                            return Err(DriverError::ConstraintNotBool {
                                rule: rule.name().to_string(),
                            });
                        }
                    }
                }
            }
        }
        Ok(Some(self))
    }
}
