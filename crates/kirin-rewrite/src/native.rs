use std::collections::HashMap;

use kirin_ir::walk::WalkResult;
use kirin_ir::{Dialect, RewriteError, Rewriter, SSAValue, StageInfo};

/// A value passed to or returned by a native function.
///
/// Pattern values bound by a match reach native functions as
/// [`NativeValue::Value`]. A function checking a condition returns a
/// [`NativeValue::Bool`] for a `wildcard.constraint` to test. An action
/// deciding how the driver goes on walking returns a [`NativeValue::WalkResult`]
/// for a `wildcard.walk_result` to read.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NativeValue {
    Bool(bool),
    Value(SSAValue),
    WalkResult(WalkResult),
}

/// A native function called while matching. It reads the IR and returns the
/// results of its `native_call`, or `None` if the match should fail, e.g.
/// because an operand is not the constant it needs.
pub type CheckFn<L> = Box<dyn Fn(&StageInfo<L>, &[NativeValue]) -> Option<Vec<NativeValue>>>;

/// A native function called while rewriting. It may edit the IR through the
/// pass's [`Rewriter`] and returns the results of its `native_call`.
pub type ActionFn<L> =
    Box<dyn Fn(&mut Rewriter<'_, L>, &[NativeValue]) -> Result<Vec<NativeValue>, RewriteError>>;

/// The native functions rules may call, by the name their `native_call`
/// stores.
///
/// That name is the path exactly as the rule wrote it, since
/// `#[rewrite_rule]` sees tokens and cannot resolve paths. One function
/// called as `has_no_uses(r)` in one rule and `checks::has_no_uses(r)` in
/// another is therefore two names here and must be registered under both.
/// A rule naming an unregistered function is rejected when it is added to a
/// [`RuleSet`](crate::RuleSet), never mid-pass.
///
/// Check functions run in the pattern body and action functions in the
/// rewrite body, so a name is looked up only in the table for where it is
/// called.
pub struct NativeRegistry<L: Dialect> {
    checks: HashMap<String, CheckFn<L>>,
    actions: HashMap<String, ActionFn<L>>,
}

impl<L: Dialect> Default for NativeRegistry<L> {
    fn default() -> Self {
        Self {
            checks: HashMap::new(),
            actions: HashMap::new(),
        }
    }
}

impl<L: Dialect> NativeRegistry<L> {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register `check` for `native_call`s named `name` in pattern bodies.
    pub fn check(
        mut self,
        name: impl Into<String>,
        check: impl Fn(&StageInfo<L>, &[NativeValue]) -> Option<Vec<NativeValue>> + 'static,
    ) -> Self {
        self.checks.insert(name.into(), Box::new(check));
        self
    }

    /// Register `action` for `native_call`s named `name` in rewrite bodies.
    pub fn action(
        mut self,
        name: impl Into<String>,
        action: impl Fn(&mut Rewriter<'_, L>, &[NativeValue]) -> Result<Vec<NativeValue>, RewriteError>
        + 'static,
    ) -> Self {
        self.actions.insert(name.into(), Box::new(action));
        self
    }

    pub(crate) fn get_check(&self, name: &str) -> Option<&CheckFn<L>> {
        self.checks.get(name)
    }

    pub(crate) fn get_action(&self, name: &str) -> Option<&ActionFn<L>> {
        self.actions.get(name)
    }
}
