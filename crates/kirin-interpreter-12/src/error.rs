use kirin_ir::SSAValue;
use std::fmt;

#[derive(Debug)]
pub enum InterpreterError {
    NoFrame,
    UnboundValue(SSAValue),
    FuelExhausted,
    MaxDepthExceeded,
    MissingEntry,
    ArityMismatch { expected: usize, got: usize },
    NoCurrent,
    UnhandledEffect(String),
    Custom(Box<dyn std::error::Error + Send + Sync>),
}

impl fmt::Display for InterpreterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoFrame => write!(f, "no active call frame"),
            Self::UnboundValue(v) => write!(f, "unbound SSA value: {v}"),
            Self::FuelExhausted => write!(f, "execution fuel exhausted"),
            Self::MaxDepthExceeded => write!(f, "maximum call depth exceeded"),
            Self::MissingEntry => write!(f, "missing stage, block, or function entry"),
            Self::ArityMismatch { expected, got } => {
                write!(f, "arity mismatch: expected {expected}, got {got}")
            }
            Self::NoCurrent => write!(f, "no current statement"),
            Self::UnhandledEffect(msg) => write!(f, "unhandled effect: {msg}"),
            Self::Custom(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for InterpreterError {}
