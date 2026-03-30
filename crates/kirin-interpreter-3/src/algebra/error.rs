use std::fmt::{Display, Formatter};

use kirin_ir::{CompileStage, Function, SSAValue, SpecializedFunction, StagedFunction};

#[derive(Debug, Clone, PartialEq, Eq, Hash, thiserror::Error)]
pub enum StageResolutionError {
    #[error("missing compile stage")]
    MissingStage,
    #[error("stage does not contain the requested dialect")]
    MissingDialect,
    #[error("typed API stage mismatch: dialect not present")]
    TypeMismatch,
    #[error("function {function:?} has no staged function mapping")]
    MissingFunction { function: Function },
    #[error("unknown function target '{name}'")]
    UnknownTarget { name: String },
    #[error("no live specialization for {staged_function:?}")]
    NoSpecialization { staged_function: StagedFunction },
    #[error("ambiguous: {count} live specializations for {staged_function:?}")]
    AmbiguousSpecialization {
        staged_function: StagedFunction,
        count: usize,
    },
    #[error("callee {callee:?} is not defined")]
    MissingCallee { callee: SpecializedFunction },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, thiserror::Error)]
pub enum MissingEntryError {
    #[error("entry block not found")]
    EntryBlock,
    #[error("block terminator not found")]
    BlockTerminator,
    #[error("function entry not found")]
    FunctionEntry,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum InterpreterError {
    #[error("no active frame")]
    NoFrame,
    #[error("unbound SSA value: {0:?}")]
    UnboundValue(SSAValue),
    #[error("{0}")]
    MissingEntry(MissingEntryError),
    #[error("no current statement")]
    NoCurrentStatement,
    #[error("arity mismatch: expected {expected} arguments, got {got}")]
    ArityMismatch { expected: usize, got: usize },
    #[error("invalid control: {0}")]
    InvalidControl(&'static str),
    #[error("unsupported interpreter behavior: {0}")]
    Unsupported(String),
    #[error("stage resolution error at {stage:?}: {kind}")]
    StageResolution {
        stage: CompileStage,
        kind: StageResolutionError,
    },
}

impl InterpreterError {
    #[must_use]
    pub fn unsupported(message: impl Into<String>) -> Self {
        Self::Unsupported(message.into())
    }

    #[must_use]
    pub const fn missing_entry_block() -> Self {
        Self::MissingEntry(MissingEntryError::EntryBlock)
    }

    #[must_use]
    pub const fn missing_terminator() -> Self {
        Self::MissingEntry(MissingEntryError::BlockTerminator)
    }

    #[must_use]
    pub const fn missing_function_entry() -> Self {
        Self::MissingEntry(MissingEntryError::FunctionEntry)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InterpError<DE> {
    Interpreter(InterpreterError),
    Dialect(DE),
}

impl<DE> From<InterpreterError> for InterpError<DE> {
    fn from(error: InterpreterError) -> Self {
        Self::Interpreter(error)
    }
}

impl<DE: Display> Display for InterpError<DE> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Interpreter(error) => Display::fmt(error, f),
            Self::Dialect(error) => Display::fmt(error, f),
        }
    }
}

impl<DE> std::error::Error for InterpError<DE> where DE: std::error::Error + 'static {}
