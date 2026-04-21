use kirin_ir::SSAValue;

#[derive(Debug)]
pub enum InterpreterError {
    UnboundValue(SSAValue),
    ArityMismatch { expected: usize, got: usize },
    UnhandledEffect(String),
    NoFrame,
    NoCurrent,
    MissingEntry,
    MaxDepthExceeded,
    Custom(Box<dyn std::error::Error + Send + Sync>),
}

impl std::fmt::Display for InterpreterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnboundValue(v) => write!(f, "unbound SSA value: {v:?}"),
            Self::ArityMismatch { expected, got } => {
                write!(f, "arity mismatch: expected {expected}, got {got}")
            }
            Self::UnhandledEffect(msg) => write!(f, "unhandled effect: {msg}"),
            Self::NoFrame => write!(f, "no active frame"),
            Self::NoCurrent => write!(f, "no current statement"),
            Self::MissingEntry => write!(f, "missing entry in pipeline/stage"),
            Self::MaxDepthExceeded => write!(f, "call depth limit exceeded"),
            Self::Custom(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for InterpreterError {}
