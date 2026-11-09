use std::fmt;

use kirin::prelude::*;

/// A call [`inline_calls`](super::inline_calls) could not inline. Calls
/// inlined before it stay inlined.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InlineError {
    /// The callee of the call is not a live specialization in this stage.
    UnknownCallee(Statement),
    /// The callee's body is not a CFG with an entry block.
    CalleeBodyNotCfg(Statement),
    /// The call passes `arguments` values to a callee whose entry block takes
    /// `parameters`.
    ArgumentCountMismatch {
        call: Statement,
        parameters: usize,
        arguments: usize,
    },
    /// A return of the callee returns `returned` values to a call with
    /// `results` results.
    ReturnCountMismatch {
        call: Statement,
        results: usize,
        returned: usize,
    },
    /// An edit failed. The checks above should prevent this.
    Rewrite(RewriteError),
}

impl From<RewriteError> for InlineError {
    fn from(error: RewriteError) -> Self {
        InlineError::Rewrite(error)
    }
}

impl fmt::Display for InlineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InlineError::UnknownCallee(call) => {
                write!(f, "the callee of {call:?} is not a live specialization")
            }
            InlineError::CalleeBodyNotCfg(call) => {
                write!(
                    f,
                    "the callee of {call:?} has no CFG body with an entry block"
                )
            }
            InlineError::ArgumentCountMismatch {
                call,
                parameters,
                arguments,
            } => write!(
                f,
                "{call:?} passes {arguments} arguments to a callee taking {parameters}"
            ),
            InlineError::ReturnCountMismatch {
                call,
                results,
                returned,
            } => write!(
                f,
                "a return of the callee of {call:?} returns {returned} values, \
                 the call has {results} results"
            ),
            InlineError::Rewrite(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for InlineError {}
