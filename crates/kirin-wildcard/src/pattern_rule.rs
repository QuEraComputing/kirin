use std::collections::HashMap;
use std::rc::Rc;

use kirin::ir::{Dialect, Rewriter, StageInfo, Statement};

use crate::{ActionFn, CheckFn, NativeError, NativeValue};

/// A `wildcard.pattern` together with the native functions its
/// `native_call`s run.
///
/// Each function is attached to `native_call`s: to one call by its
/// statement, which is how `#[rewrite_rule]` attaches the function each
/// call names, or to every call with a name, which is shorter for a pattern
/// built by hand. Either way each call must end up with exactly one
/// function, which is checked when the rule is added to a rule set. Every
/// function given is kept, so a call with two is rejected there rather than
/// one replacing the other.
///
/// The functions belong to this rule alone, so two rules may use the same
/// name for different functions. A `native_call` in the pattern body runs a
/// check and one in the rewrite body an action.
///
/// A pattern calling no native functions converts from its [`Statement`].
pub struct PatternRule<L: Dialect> {
    pattern: Statement,
    /// The checks attached to one call each, by its statement.
    checks_at: HashMap<Statement, Vec<CheckFn<L>>>,
    /// The checks attached to every call with a name, by the name.
    checks_named: HashMap<String, Vec<CheckFn<L>>>,
    /// The actions attached to one call each, by its statement.
    actions_at: HashMap<Statement, Vec<ActionFn<L>>>,
    /// The actions attached to every call with a name, by the name.
    actions_named: HashMap<String, Vec<ActionFn<L>>>,
}

impl<L: Dialect> PatternRule<L> {
    /// The rule for the `wildcard.pattern` statement `pattern`, with no
    /// native functions yet.
    pub fn new(pattern: Statement) -> Self {
        Self {
            pattern,
            checks_at: HashMap::new(),
            checks_named: HashMap::new(),
            actions_at: HashMap::new(),
            actions_named: HashMap::new(),
        }
    }

    /// Attach function `check` to every `native_call` named `name` in the
    /// pattern body.
    pub fn check(
        mut self,
        name: impl Into<String>,
        check: impl Fn(&StageInfo<L>, &[NativeValue]) -> Result<Option<Vec<NativeValue>>, NativeError>
        + 'static,
    ) -> Self {
        let check: CheckFn<L> = Rc::new(check);
        self.checks_named
            .entry(name.into())
            .or_default()
            .push(check);
        self
    }

    /// Attach function `check` to the `native_call` `call` in the pattern
    /// body.
    pub fn check_at(
        mut self,
        call: Statement,
        check: impl Fn(&StageInfo<L>, &[NativeValue]) -> Result<Option<Vec<NativeValue>>, NativeError>
        + 'static,
    ) -> Self {
        let check: CheckFn<L> = Rc::new(check);
        self.checks_at.entry(call).or_default().push(check);
        self
    }

    /// Attach function `action` to every `native_call` named `name` in the
    /// rewrite body.
    pub fn action(
        mut self,
        name: impl Into<String>,
        action: impl Fn(&mut Rewriter<'_, L>, &[NativeValue]) -> Result<Vec<NativeValue>, NativeError>
        + 'static,
    ) -> Self {
        let action: ActionFn<L> = Rc::new(action);
        self.actions_named
            .entry(name.into())
            .or_default()
            .push(action);
        self
    }

    /// Attach function `action` to the `native_call` `call` in the rewrite
    /// body.
    pub fn action_at(
        mut self,
        call: Statement,
        action: impl Fn(&mut Rewriter<'_, L>, &[NativeValue]) -> Result<Vec<NativeValue>, NativeError>
        + 'static,
    ) -> Self {
        let action: ActionFn<L> = Rc::new(action);
        self.actions_at.entry(call).or_default().push(action);
        self
    }

    /// The `wildcard.pattern` statement.
    pub fn pattern(&self) -> Statement {
        self.pattern
    }

    /// The checks attached to the `native_call` `call`, named `name`.
    pub fn checks_of(&self, call: Statement, name: &str) -> impl Iterator<Item = &CheckFn<L>> {
        let at = self.checks_at.get(&call).into_iter().flatten();
        at.chain(self.checks_named.get(name).into_iter().flatten())
    }

    /// The actions attached to the `native_call` `call`, named `name`.
    pub fn actions_of(&self, call: Statement, name: &str) -> impl Iterator<Item = &ActionFn<L>> {
        let at = self.actions_at.get(&call).into_iter().flatten();
        at.chain(self.actions_named.get(name).into_iter().flatten())
    }
}

impl<L: Dialect> From<Statement> for PatternRule<L> {
    fn from(pattern: Statement) -> Self {
        Self::new(pattern)
    }
}
