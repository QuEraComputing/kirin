use kirin_ir::{CompileStage, Function, SSAValue, SpecializedFunction, StagedFunction};

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
    /// The requested stage does not exist in the pipeline.
    #[error("missing compile stage: {stage:?}")]
    MissingStage { stage: CompileStage },
    /// The stage exists but does not contain the requested dialect.
    #[error("stage {stage:?} does not contain the requested dialect")]
    MissingStageDialect { stage: CompileStage },
    /// Typed API was called with a dialect not present in the current frame stage.
    #[error(
        "typed API stage mismatch: frame stage {frame_stage:?} does not contain requested dialect"
    )]
    TypedStageMismatch { frame_stage: CompileStage },
    /// Function has no staged-function mapping for the requested stage.
    #[error("function {function:?} has no staged function for stage {stage:?}")]
    MissingFunctionStageMapping {
        function: Function,
        stage: CompileStage,
    },
    /// No abstract function with the requested symbolic name exists.
    #[error("unknown function target '{name}' at stage {stage:?}")]
    UnknownFunctionTarget { name: String, stage: CompileStage },
    /// No live specialization exists for the requested staged function/stage pair.
    #[error("no live specialization for staged function {staged_function:?} at stage {stage:?}")]
    NoSpecializationAtStage {
        staged_function: StagedFunction,
        stage: CompileStage,
    },
    /// More than one live specialization exists when unique-or-error is required.
    #[error(
        "ambiguous live specializations for staged function {staged_function:?} at stage {stage:?}: {count}"
    )]
    AmbiguousSpecializationAtStage {
        staged_function: StagedFunction,
        stage: CompileStage,
        count: usize,
    },
    /// A continuation referred to a callee that does not exist at the given stage.
    #[error("callee {callee:?} is not defined at stage {stage:?}")]
    MissingCalleeAtStage {
        callee: SpecializedFunction,
        stage: CompileStage,
    },
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
