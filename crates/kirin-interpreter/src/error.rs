use kirin_ir::SSAValue;

/// Error type for interpreter failures.
///
/// Framework errors cover common interpreter failure modes. User-defined
/// errors (e.g. division by zero, type errors) go in the [`Custom`](Self::Custom)
/// variant via [`InterpreterError::custom`].
#[derive(Debug, thiserror::Error)]
pub enum InterpreterError {
    /// No call frame on the stack.
    #[error("no active call frame")]
    NoFrame,
    /// An SSA value was read before being written.
    #[error("unbound SSA value: {0:?}")]
    UnboundValue(SSAValue),
    /// Step fuel has been exhausted.
    #[error("step fuel exhausted")]
    FuelExhausted,
    /// Call depth exceeded the configured maximum.
    #[error("call depth exceeded maximum")]
    MaxDepthExceeded,
    /// Function entry block could not be resolved.
    #[error("function entry block not found")]
    MissingEntry,
    /// An unexpected control flow action was encountered.
    #[error("unexpected control flow: {0}")]
    UnexpectedControl(String),
    /// Argument count does not match block/function parameter count.
    #[error("arity mismatch: expected {expected} arguments, got {got}")]
    ArityMismatch { expected: usize, got: usize },
    /// User-defined error.
    #[error(transparent)]
    Custom(Box<dyn std::error::Error + Send + Sync>),
}

impl InterpreterError {
    /// Wrap an arbitrary error as [`InterpreterError::Custom`].
    pub fn custom(error: impl std::error::Error + Send + Sync + 'static) -> Self {
        InterpreterError::Custom(Box::new(error))
    }
}
