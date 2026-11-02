use std::collections::HashMap;
use std::rc::Rc;

use kirin::ir::{Dialect, Rewriter, StageInfo, Statement};

use crate::{ActionFn, CheckFn, NativeError, NativeValue};

/// A `wildcard.pattern` together with the native functions its
/// `native_call`s name.
///
/// The functions belong to this rule alone, so two rules may use the same
/// name for different functions. A `native_call` in the pattern body is
/// looked up among the checks and one in the rewrite body among the actions.
///
/// A pattern calling no native functions converts from its [`Statement`].
pub struct PatternRule<L: Dialect> {
    pattern: Statement,
    checks: HashMap<String, CheckFn<L>>,
    actions: HashMap<String, ActionFn<L>>,
}

impl<L: Dialect> PatternRule<L> {
    /// The rule for the `wildcard.pattern` statement `pattern`, with no
    /// native functions yet.
    pub fn new(pattern: Statement) -> Self {
        Self {
            pattern,
            checks: HashMap::new(),
            actions: HashMap::new(),
        }
    }

    /// Run `check` for `native_call`s named `name` in the pattern body.
    pub fn check(
        mut self,
        name: impl Into<String>,
        check: impl Fn(&StageInfo<L>, &[NativeValue]) -> Result<Option<Vec<NativeValue>>, NativeError>
        + 'static,
    ) -> Self {
        self.checks.insert(name.into(), Rc::new(check));
        self
    }

    /// Run `action` for `native_call`s named `name` in the rewrite body.
    pub fn action(
        mut self,
        name: impl Into<String>,
        action: impl Fn(&mut Rewriter<'_, L>, &[NativeValue]) -> Result<Vec<NativeValue>, NativeError>
        + 'static,
    ) -> Self {
        self.actions.insert(name.into(), Rc::new(action));
        self
    }

    /// The `wildcard.pattern` statement.
    pub fn pattern(&self) -> Statement {
        self.pattern
    }

    /// The check named `name`.
    pub fn check_fn(&self, name: &str) -> Option<&CheckFn<L>> {
        self.checks.get(name)
    }

    /// The action named `name`.
    pub fn action_fn(&self, name: &str) -> Option<&ActionFn<L>> {
        self.actions.get(name)
    }
}

impl<L: Dialect> From<Statement> for PatternRule<L> {
    fn from(pattern: Statement) -> Self {
        Self::new(pattern)
    }
}
