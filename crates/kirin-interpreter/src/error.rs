use std::fmt;

use kirin_ir::SSAValue;

use crate::InterpreterError;

/// Default error type for interpreter failures.
///
/// Covers the error conditions required by [`InterpreterError`].
/// Users who need additional error variants (e.g. division by zero,
/// type errors) should define their own error type and implement
/// [`InterpreterError`] for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InterpError {
    /// No call frame on the stack.
    NoFrame,
    /// An SSA value was read before being written.
    UnboundValue(SSAValue),
    /// Step fuel has been exhausted.
    FuelExhausted,
    /// Call depth exceeded the configured maximum.
    MaxDepthExceeded,
    /// Function entry block could not be resolved.
    MissingEntry,
    /// An unexpected control flow action was encountered.
    UnexpectedControl(String),
}

impl InterpreterError for InterpError {
    fn no_frame() -> Self {
        InterpError::NoFrame
    }

    fn unbound_value(value: SSAValue) -> Self {
        InterpError::UnboundValue(value)
    }

    fn fuel_exhausted() -> Self {
        InterpError::FuelExhausted
    }

    fn max_depth_exceeded() -> Self {
        InterpError::MaxDepthExceeded
    }

    fn missing_entry() -> Self {
        InterpError::MissingEntry
    }

    fn unexpected_control(msg: &str) -> Self {
        InterpError::UnexpectedControl(msg.to_owned())
    }
}

impl fmt::Display for InterpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InterpError::NoFrame => write!(f, "no active call frame"),
            InterpError::UnboundValue(v) => write!(f, "unbound SSA value: {v:?}"),
            InterpError::FuelExhausted => write!(f, "step fuel exhausted"),
            InterpError::MaxDepthExceeded => write!(f, "call depth exceeded maximum"),
            InterpError::MissingEntry => write!(f, "function entry block not found"),
            InterpError::UnexpectedControl(msg) => {
                write!(f, "unexpected control flow: {msg}")
            }
        }
    }
}

impl std::error::Error for InterpError {}
