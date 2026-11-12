use kirin_ir::{BuilderStageInfo, Dialect, StageInfo};
use kirin_wildcard::{PatternLanguage, PatternRule};

use super::{BlockRule, CheckedRule, NativeRule, RuleError, RuleErrorKind, StatementRule};

/// A rule of a [`RuleSet`]: a checked pattern or a native rule's recipe.
pub(crate) enum Rule<L: Dialect> {
    Pattern(CheckedRule<L>),
    Statement(NativeRule<dyn StatementRule<L>>),
    Block(NativeRule<dyn BlockRule<L>>),
}

/// The rules one driver run tries, in order, with the stage holding their
/// patterns.
///
/// Declarative rules (patterns) and native rules (written in Rust, see
/// [`StatementRule`] and [`BlockRule`]) share one list: the order rules are
/// added in, of either kind, is the order the driver tries them at each
/// statement.
pub struct RuleSet<L: Dialect> {
    patterns: StageInfo<PatternLanguage<L>>,
    rules: Vec<Rule<L>>,
}

impl<L: Dialect> RuleSet<L> {
    /// A rule set over the patterns in `patterns`. Add its rules with
    /// [`RuleSet::add_pattern_rule`].
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
            set.add_pattern_rule(rule)?;
        }
        Ok(set)
    }

    /// Check `rule`'s pattern and add it as the last rule to try.
    pub fn add_pattern_rule(
        &mut self,
        rule: impl Into<PatternRule<L>>,
    ) -> Result<&mut Self, RuleError> {
        let rule = CheckedRule::new(&self.patterns, rule.into())?;
        self.rules.push(Rule::Pattern(rule));
        Ok(self)
    }

    /// Add a native statement rule as the last rule to try. `build` makes a
    /// new copy of the rule, and the driver calls it at the start of every
    /// sweep. `name` identifies the rule in errors.
    pub fn add_statement_rule<R: StatementRule<L> + 'static>(
        &mut self,
        name: impl Into<String>,
        build: impl Fn() -> R + 'static,
    ) -> &mut Self {
        self.rules.push(Rule::Statement(NativeRule::new(
            name.into(),
            Box::new(move || Box::new(build())),
        )));
        self
    }

    /// Add a native block rule as the last rule to try. `build` makes a new
    /// copy of the rule, and the driver calls it for every block. `name`
    /// identifies the rule in errors.
    pub fn add_block_rule<R: BlockRule<L> + 'static>(
        &mut self,
        name: impl Into<String>,
        build: impl Fn() -> R + 'static,
    ) -> &mut Self {
        self.rules.push(Rule::Block(NativeRule::new(
            name.into(),
            Box::new(move || Box::new(build())),
        )));
        self
    }

    /// The rules, in the order they are tried.
    pub(crate) fn rules(&self) -> &[Rule<L>] {
        &self.rules
    }

    /// The stage the rules' patterns live in.
    pub fn patterns(&self) -> &StageInfo<PatternLanguage<L>> {
        &self.patterns
    }
}
