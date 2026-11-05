use kirin_ir::{BuilderStageInfo, Dialect, StageInfo};
use kirin_wildcard::{PatternLanguage, PatternRule};

use super::{CheckedRule, RuleError, RuleErrorKind};

/// The rules one driver run tries, in order, with the stage holding their
/// patterns.
///
/// The order rules are added in is the order the driver tries them at each
/// statement.
pub struct RuleSet<L: Dialect> {
    patterns: StageInfo<PatternLanguage<L>>,
    rules: Vec<CheckedRule<L>>,
}

impl<L: Dialect> RuleSet<L> {
    /// A rule set over the patterns in `patterns`. Add its rules with
    /// [`RuleSet::add`].
    pub fn new(patterns: StageInfo<PatternLanguage<L>>) -> Self {
        Self {
            patterns,
            rules: Vec::new(),
        }
    }

    /// Build every pattern with `build`, which returns the rules in the order
    /// to try them, then add them all.
    ///
    /// `build` is typically a list of `#[rewrite_rule]` functions:
    /// `|stage| vec![double_neg(stage), dead_neg(stage)]`.
    pub fn build<R: Into<PatternRule<L>>>(
        build: impl FnOnce(&mut BuilderStageInfo<PatternLanguage<L>>) -> Vec<R>,
    ) -> Result<Self, RuleError> {
        let mut stage = BuilderStageInfo::default();
        let rules = build(&mut stage);
        let stage = stage.finalize().map_err(|error| RuleError {
            rule: None,
            kind: RuleErrorKind::Finalize(error),
        })?;
        let mut set = Self::new(stage);
        for rule in rules {
            set.add(rule)?;
        }
        Ok(set)
    }

    /// Check `rule`'s pattern and add it as the last rule to try.
    pub fn add(&mut self, rule: impl Into<PatternRule<L>>) -> Result<&mut Self, RuleError> {
        let rule = CheckedRule::new(&self.patterns, rule.into())?;
        self.rules.push(rule);
        Ok(self)
    }

    /// The rules, in the order they are tried.
    pub fn rules(&self) -> &[CheckedRule<L>] {
        &self.rules
    }

    /// The stage the rules' patterns live in.
    pub fn patterns(&self) -> &StageInfo<PatternLanguage<L>> {
        &self.patterns
    }
}
