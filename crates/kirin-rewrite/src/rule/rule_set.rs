use kirin_ir::{BuilderStageInfo, Dialect, StageInfo};
use kirin_wildcard::{PatternLanguage, PatternRule};

use super::{BlockRule, CheckedRule, NativeRule, RuleError, RuleErrorKind, StatementRule};
use crate::{AnalysisCache, AnalysisKind, Requirement};

/// A rule of a [`RuleSet`]: a checked pattern or a native rule's recipe.
pub(crate) enum Rule<L: Dialect> {
    Pattern(CheckedRule<L>),
    Statement(NativeRule<dyn StatementRule<L>>),
    Block(NativeRule<dyn BlockRule<L>>),
}

impl<L: Dialect> Rule<L> {
    /// The rule's name, used in errors.
    pub(crate) fn name(&self) -> &str {
        match self {
            Rule::Pattern(rule) => rule.name(),
            Rule::Statement(native) => native.name(),
            Rule::Block(native) => native.name(),
        }
    }

    /// The analysis results the rule reads. Pattern rules read none.
    pub(crate) fn requirements(&self) -> &[Requirement] {
        match self {
            Rule::Pattern(_) => &[],
            Rule::Statement(native) => native.requirements(),
            Rule::Block(native) => native.requirements(),
        }
    }
}

/// A native rule just added to a [`RuleSet`], to declare the analysis results
/// its recipe reads.
pub struct AddedRule<'a> {
    requirements: &'a mut Vec<Requirement>,
}

impl AddedRule<'_> {
    /// Declare that the rule reads analysis `A`'s result for `key`, so that
    /// [`RuleSet::required`] lists it and a pass computes it first. A rule
    /// that reads several results declares each.
    pub fn requires<A: AnalysisKind>(&mut self, key: A::Key) -> &mut Self {
        self.requirements.push(Requirement::of::<A>(key));
        self
    }
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
    /// new copy of the rule from the analysis results the pass was given, and
    /// the driver calls it at the start of every sweep. `name` identifies the
    /// rule in errors. Declare the results `build` reads with
    /// [`AddedRule::requires`].
    pub fn add_statement_rule<R: StatementRule<L> + 'static>(
        &mut self,
        name: impl Into<String>,
        build: impl Fn(&AnalysisCache) -> R + 'static,
    ) -> AddedRule<'_> {
        self.rules.push(Rule::Statement(NativeRule::new(
            name.into(),
            Box::new(move |cache| Box::new(build(cache))),
        )));
        self.last_added()
    }

    /// Add a native block rule as the last rule to try. `build` makes a new
    /// copy of the rule from the analysis results the pass was given, and the
    /// driver calls it for every block. `name` identifies the rule in errors.
    /// Declare the results `build` reads with [`AddedRule::requires`].
    pub fn add_block_rule<R: BlockRule<L> + 'static>(
        &mut self,
        name: impl Into<String>,
        build: impl Fn(&AnalysisCache) -> R + 'static,
    ) -> AddedRule<'_> {
        self.rules.push(Rule::Block(NativeRule::new(
            name.into(),
            Box::new(move |cache| Box::new(build(cache))),
        )));
        self.last_added()
    }

    /// The native rule just added.
    fn last_added(&mut self) -> AddedRule<'_> {
        let requirements = match self.rules.last_mut() {
            Some(Rule::Statement(native)) => native.requirements_mut(),
            Some(Rule::Block(native)) => native.requirements_mut(),
            _ => unreachable!("called right after adding a native rule"),
        };
        AddedRule { requirements }
    }

    /// Every rule's [declared](AddedRule::requires) analysis results, in
    /// rule order, for [`Database::run_pass`](crate::Database::run_pass). A
    /// result two rules read is listed twice; computing it the second time
    /// is a cache hit.
    pub fn required(&self) -> Vec<Requirement> {
        self.rules
            .iter()
            .flat_map(|rule| rule.requirements())
            .cloned()
            .collect()
    }

    /// The rules, in the order they are tried.
    pub(crate) fn rules(&self) -> &[Rule<L>] {
        &self.rules
    }

    /// Whether any rule is a native statement or block rule.
    pub(crate) fn has_native_rules(&self) -> bool {
        self.rules
            .iter()
            .any(|rule| !matches!(rule, Rule::Pattern(_)))
    }

    /// The stage the rules' patterns live in.
    pub fn patterns(&self) -> &StageInfo<PatternLanguage<L>> {
        &self.patterns
    }
}
