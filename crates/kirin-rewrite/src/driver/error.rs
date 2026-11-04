use std::fmt;

use kirin_ir::walk::WalkError;
use kirin_ir::{RewriteError, Statement};
use kirin_wildcard::{NativeError, NativeFailure};

/// Why a driver run failed.
///
/// Edits made before the failure stay applied. Inside
/// [`Pipeline::run_pass`](kirin_ir::Pipeline::run_pass), returning this
/// quarantines the stage.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DriverError {
    /// An action of `rule` matched at `stmt` was rejected by the
    /// [`Rewriter`](kirin_ir::Rewriter).
    Rewrite {
        rule: String,
        stmt: Statement,
        error: RewriteError,
    },
    /// A native function returned a different number of values than its
    /// `native_call` has results.
    NativeArity {
        rule: String,
        name: String,
        expected: usize,
        found: usize,
    },
    /// Native function `name` was given an argument of the wrong type.
    ///
    /// - `position`: which argument, counting from 0
    /// - `expected`: the type the function takes, e.g. `"i64"`
    /// - `found`: the value it was given, printed with `Debug`, or `None` if
    ///   the call has fewer arguments than that
    NativeArgument {
        rule: String,
        name: String,
        position: usize,
        expected: &'static str,
        found: Option<String>,
    },
    /// Native function `name`, called by `rule` matched at `stmt`, returned
    /// `error`. [`NativeFailure::error`] gives the error back.
    NativeFailed {
        rule: String,
        name: String,
        stmt: Statement,
        error: NativeFailure,
    },
    /// Native function `name`, called with `?` in the rewrite body of `rule`
    /// matched at `stmt`, returned `None`.
    NativeReturnedNone {
        rule: String,
        name: String,
        stmt: Statement,
    },
    /// A value the rule reads was never bound, such as a hole no matched op
    /// uses.
    Unbound { rule: String },
    /// A `constraint` tested a value that is not a `bool`.
    ConstraintNotBool { rule: String },
    /// A `replace` redirected to or from a value that is not an SSA value.
    NotAnSsaValue { rule: String },
    /// Rule `rule` erases its root, the statement being visited, which an
    /// `order` walk does not allow. Nothing was edited.
    ErasesRoot { rule: String, order: &'static str },
    /// The tree under `root` was still changing after `sweeps` sweeps, the
    /// driver's limit: the rules most likely keep undoing each other.
    BudgetExhausted { root: Statement, sweeps: usize },
    /// An edit broke the walk's contract, such as a native action erasing the
    /// statement being visited.
    Walk(WalkError),
    /// The dead-code cleanup could not erase `stmt`.
    DeadCode {
        stmt: Statement,
        error: RewriteError,
    },
}

impl DriverError {
    /// The failure of native function `name`, called by `rule` matched at
    /// `stmt`.
    pub(crate) fn native(rule: &str, name: &str, stmt: Statement, error: NativeError) -> Self {
        match error {
            NativeError::Argument {
                position,
                expected,
                found,
            } => DriverError::NativeArgument {
                rule: rule.to_string(),
                name: name.to_string(),
                position,
                expected,
                found,
            },
            NativeError::Rewrite(error) => DriverError::Rewrite {
                rule: rule.to_string(),
                stmt,
                error,
            },
            NativeError::Failed(error) => DriverError::NativeFailed {
                rule: rule.to_string(),
                name: name.to_string(),
                stmt,
                error,
            },
            NativeError::NoValue => DriverError::NativeReturnedNone {
                rule: rule.to_string(),
                name: name.to_string(),
                stmt,
            },
        }
    }
}

impl From<WalkError> for DriverError {
    fn from(error: WalkError) -> Self {
        DriverError::Walk(error)
    }
}

impl fmt::Display for DriverError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DriverError::Rewrite { rule, stmt, error } => {
                write!(
                    f,
                    "rule `{rule}` matched at {stmt:?} but its rewrite failed: {error}"
                )
            }
            DriverError::NativeArity {
                rule,
                name,
                expected,
                found,
            } => write!(
                f,
                "rule `{rule}`: native function `{name}` returned {found} values, expected {expected}"
            ),
            DriverError::NativeArgument {
                rule,
                name,
                position,
                expected,
                found: Some(found),
            } => write!(
                f,
                "rule `{rule}`: argument {position} of native function `{name}` is {found}, expected a `{expected}`"
            ),
            DriverError::NativeArgument {
                rule,
                name,
                position,
                expected,
                found: None,
            } => write!(
                f,
                "rule `{rule}`: native function `{name}` has no argument {position}, expected a `{expected}`"
            ),
            DriverError::NativeFailed {
                rule,
                name,
                stmt,
                error,
            } => write!(
                f,
                "rule `{rule}` matched at {stmt:?} but native function `{name}` failed: {error}"
            ),
            DriverError::NativeReturnedNone { rule, name, stmt } => write!(
                f,
                "rule `{rule}` matched at {stmt:?} but native function `{name}` returned `None` in its rewrite body"
            ),
            DriverError::Unbound { rule } => {
                write!(f, "rule `{rule}` reads a value the match did not bind")
            }
            DriverError::ConstraintNotBool { rule } => {
                write!(
                    f,
                    "rule `{rule}`: a constraint tested a value that is not a bool"
                )
            }
            DriverError::NotAnSsaValue { rule } => {
                write!(
                    f,
                    "rule `{rule}`: a replace names a value that is not an SSA value"
                )
            }
            DriverError::ErasesRoot { rule, order } => write!(
                f,
                "rule `{rule}` erases its root, the statement being visited, which a `{order}` walk does not allow; run it in an order that does, such as `ReverseBFS`"
            ),
            DriverError::BudgetExhausted { root, sweeps } => write!(
                f,
                "the IR under {root:?} was still changing after {sweeps} sweeps; \
                 the rules may never reach a fixpoint"
            ),
            DriverError::Walk(error) => write!(f, "the walk failed: {error}"),
            DriverError::DeadCode { stmt, error } => {
                write!(f, "dead-code elimination could not erase {stmt:?}: {error}")
            }
        }
    }
}

impl std::error::Error for DriverError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            DriverError::NativeFailed { error, .. } => Some(error.error()),
            _ => None,
        }
    }
}
