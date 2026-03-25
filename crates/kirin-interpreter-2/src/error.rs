use kirin_ir::{CompileStage, Function, SSAValue, SpecializedFunction, StagedFunction};

/// Detailed reason for a stage-resolution failure.
#[derive(Debug, thiserror::Error)]
pub enum StageResolutionError {
    #[error("missing compile stage")]
    MissingStage,
    #[error("stage does not contain the requested dialect")]
    TypeMismatch,
    #[error("unknown target: {name}")]
    UnknownTarget { name: String },
    #[error("missing function: {function:?}")]
    MissingFunction { function: Function },
    #[error("missing callee specialization info: {callee:?}")]
    MissingCallee { callee: SpecializedFunction },
    #[error("no live specialization for staged function: {staged_function:?}")]
    NoSpecialization { staged_function: StagedFunction },
    #[error(
        "ambiguous specialization for staged function {staged_function:?}: {count} live matches"
    )]
    AmbiguousSpecialization {
        staged_function: StagedFunction,
        count: usize,
    },
}

/// Detailed reason for a missing-entry failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, thiserror::Error)]
pub enum MissingEntryError {
    #[error("entry block not found")]
    EntryBlock,
    #[error("block terminator not found")]
    BlockTerminator,
}

/// Baseline runtime errors for the new interpreter family.
#[derive(Debug, thiserror::Error)]
pub enum InterpreterError {
    #[error("no active frame")]
    NoFrame,
    #[error("no current statement")]
    NoCurrentStatement,
    #[error("unbound SSA value: {0:?}")]
    UnboundValue(SSAValue),
    #[error("step fuel exhausted")]
    FuelExhausted,
    #[error("call depth exceeded maximum")]
    MaxDepthExceeded,
    #[error("{0}")]
    MissingEntry(MissingEntryError),
    #[error("arity mismatch: expected {expected} arguments, got {got}")]
    ArityMismatch { expected: usize, got: usize },
    #[error("stage resolution error at {stage:?}: {kind}")]
    StageResolution {
        stage: CompileStage,
        kind: StageResolutionError,
    },
    #[error("invalid shell control transition: {0}")]
    InvalidControl(&'static str),
    #[error(transparent)]
    Custom(Box<dyn std::error::Error + Send + Sync>),
}

impl InterpreterError {
    pub fn custom(error: impl std::error::Error + Send + Sync + 'static) -> Self {
        Self::Custom(Box::new(error))
    }

    pub fn missing_entry_block() -> Self {
        Self::MissingEntry(MissingEntryError::EntryBlock)
    }

    pub fn missing_terminator() -> Self {
        Self::MissingEntry(MissingEntryError::BlockTerminator)
    }
}
