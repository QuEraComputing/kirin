use kirin_ir::{CompileStage, Function, SSAValue, SpecializedFunction, StagedFunction};

/// Detailed reason for a stage/pipeline resolution failure.
#[derive(Debug, thiserror::Error)]
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

/// Detailed reason for a missing entry failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, thiserror::Error)]
pub enum MissingEntryError {
    /// The region has no entry block.
    #[error("entry block not found")]
    EntryBlock,
    /// A block has no terminator statement.
    #[error("block terminator not found")]
    BlockTerminator,
    /// A callable body did not resolve to a function entry.
    #[error("function entry not found")]
    FunctionEntry,
}

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
    /// An entry-related lookup failed.
    #[error("{0}")]
    MissingEntry(MissingEntryError),
    /// Argument count does not match block/function parameter count.
    #[error("arity mismatch: expected {expected} arguments, got {got}")]
    ArityMismatch { expected: usize, got: usize },
    /// Stage or pipeline resolution failure.
    #[error("stage resolution error at {stage:?}: {kind}")]
    StageResolution {
        stage: CompileStage,
        kind: StageResolutionError,
    },
    /// An unexpected control flow action was encountered.
    #[error("unexpected control flow: {0}")]
    UnexpectedControl(String),
    /// User-defined error.
    #[error(transparent)]
    Custom(Box<dyn std::error::Error + Send + Sync>),
}

impl InterpreterError {
    /// Wrap an arbitrary error as [`InterpreterError::Custom`].
    pub fn custom(error: impl std::error::Error + Send + Sync + 'static) -> Self {
        InterpreterError::Custom(Box::new(error))
    }

    /// The region has no entry block.
    pub fn missing_entry_block() -> Self {
        Self::MissingEntry(MissingEntryError::EntryBlock)
    }

    /// A block has no terminator statement.
    pub fn missing_terminator() -> Self {
        Self::MissingEntry(MissingEntryError::BlockTerminator)
    }

    /// A callable body did not resolve to a function entry.
    pub fn missing_function_entry() -> Self {
        Self::MissingEntry(MissingEntryError::FunctionEntry)
    }
}
