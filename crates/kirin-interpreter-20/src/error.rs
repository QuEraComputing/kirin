use kirin_ir::SSAValue;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum InterpreterError {
    #[error("unbound SSA value: {0:?}")]
    UnboundValue(SSAValue),
    #[error("arity mismatch: expected {expected}, got {got}")]
    ArityMismatch { expected: usize, got: usize },
    #[error("unhandled effect: {0}")]
    UnhandledEffect(String),
    #[error("no active frame")]
    NoFrame,
    #[error("no current statement")]
    NoCurrent,
    #[error("missing entry in pipeline/stage")]
    MissingEntry,
    #[error("call depth limit exceeded")]
    MaxDepthExceeded,
    #[error("fuel exhausted")]
    FuelExhausted,
    #[error("{0}")]
    Custom(Box<dyn std::error::Error + Send + Sync>),
}
