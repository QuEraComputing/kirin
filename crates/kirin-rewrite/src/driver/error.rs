use std::fmt;

use kirin_ir::walk::WalkError;
use kirin_ir::{RewriteError, Statement};

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
    /// A value the rule reads was never bound, such as a hole no matched op
    /// uses.
    Unbound { rule: String },
    /// A `constraint` tested a value that is not a [`NativeValue::Bool`](crate::NativeValue::Bool).
    ConstraintNotBool { rule: String },
    /// A `replace` redirected to or from a value that is not an SSA value.
    NotAnSsaValue { rule: String },
    /// A `walk_result` read a value that is not a
    /// [`NativeValue::WalkResult`](crate::NativeValue::WalkResult).
    NotAWalkResult { rule: String },
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
            DriverError::NotAWalkResult { rule } => {
                write!(
                    f,
                    "rule `{rule}`: a walk result reads a value that is not a walk result"
                )
            }
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

impl std::error::Error for DriverError {}
