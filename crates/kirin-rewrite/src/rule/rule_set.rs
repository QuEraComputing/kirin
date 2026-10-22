use kirin_ir::{BuilderStageInfo, Dialect, StageInfo, Statement};
use kirin_wildcard::PatternLanguage;

use super::{RewriteRule, RuleError, RuleErrorKind};
use crate::NativeRegistry;

/// The rules one driver run tries, in order, with the stage holding their
/// patterns and the native functions they call.
///
/// The order rules are added in is the order the driver tries them at each
/// statement.
pub struct RuleSet<L: Dialect> {
    patterns: StageInfo<PatternLanguage<L>>,
    rules: Vec<RewriteRule>,
    natives: NativeRegistry<L>,
}

impl<L: Dialect> RuleSet<L> {
    /// A rule set over the patterns in `patterns`, calling `natives`. Add
    /// its rules with [`RuleSet::add`].
    pub fn new(patterns: StageInfo<PatternLanguage<L>>, natives: NativeRegistry<L>) -> Self {
        Self {
            patterns,
            rules: Vec::new(),
            natives,
        }
    }

    /// Build every pattern with `build`, which returns the `wildcard.pattern`
    /// statements in the order to try them, then add them all.
    ///
    /// `build` is typically a list of `#[rewrite_rule]` functions:
    /// `|stage| vec![double_neg(stage), dead_neg(stage)]`.
    pub fn build(
        natives: NativeRegistry<L>,
        build: impl FnOnce(&mut BuilderStageInfo<PatternLanguage<L>>) -> Vec<Statement>,
    ) -> Result<Self, RuleError> {
        let mut stage = BuilderStageInfo::default();
        let patterns = build(&mut stage);
        let stage = stage.finalize().map_err(|error| RuleError {
            rule: None,
            kind: RuleErrorKind::Finalize(error),
        })?;
        let mut rules = Self::new(stage, natives);
        for pattern in patterns {
            rules.add(pattern)?;
        }
        Ok(rules)
    }

    /// Check the `wildcard.pattern` statement `pattern` and add it as the
    /// last rule to try.
    pub fn add(&mut self, pattern: Statement) -> Result<&mut Self, RuleError> {
        let rule = RewriteRule::new(&self.patterns, &self.natives, pattern)?;
        self.rules.push(rule);
        Ok(self)
    }

    /// The rules, in the order they are tried.
    pub fn rules(&self) -> &[RewriteRule] {
        &self.rules
    }

    /// The stage the rules' patterns live in.
    pub fn patterns(&self) -> &StageInfo<PatternLanguage<L>> {
        &self.patterns
    }

    pub(crate) fn natives(&self) -> &NativeRegistry<L> {
        &self.natives
    }
}
