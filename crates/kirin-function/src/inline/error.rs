use std::fmt;

use kirin::prelude::*;

/// A call [`inline_calls`](super::inline_calls) found whose callee is not a
/// live specialization, or whose argument or result count does not match the
/// callee's signature. Every call is checked before any edit, so the stage is
/// left as it was.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InlineError {
    /// The callee of the call is not a live specialization in this stage.
    UnknownCallee(Statement),
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
}

impl fmt::Display for InlineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InlineError::UnknownCallee(call) => {
                write!(f, "the callee of {call:?} is not a live specialization")
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
        }
    }
}

impl std::error::Error for InlineError {}
