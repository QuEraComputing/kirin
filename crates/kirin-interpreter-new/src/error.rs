use kirin_ir::{CompileStage, Function, SSAValue, SpecializedFunction, StagedFunction, Symbol};
use thiserror::Error;

use crate::{EnvIndex, Location};

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum InterpreterError {
    #[error("invalid environment index {0:?}")]
    InvalidEnvIndex(EnvIndex),
    #[error("unbound SSA value {value} in environment {index:?}")]
    UnboundValue { index: EnvIndex, value: SSAValue },
    #[error("environment stack is empty")]
    EmptyEnvStack,
    #[error("frame stack is empty")]
    EmptyFrameStack,
    #[error("missing stage {0:?}")]
    MissingStage(CompileStage),
    #[error("missing stage info for stage {0:?}")]
    MissingStageInfo(CompileStage),
    #[error("expected an active statement at {0:?}")]
    ExpectedActiveStatement(Location),
    #[error("expected an active block at {0:?}")]
    ExpectedActiveBlock(Location),
    #[error("statement frame cannot be stepped directly at {0:?}")]
    UnexpectedStatementFrameStep(Location),
    #[error("unexpected completion at {location:?}: {completion}")]
    UnexpectedCompletion {
        location: Location,
        completion: &'static str,
    },
    #[error("missing function {0:?}")]
    MissingFunction(Function),
    #[error("function {function:?} has no staged function for stage {stage:?}")]
    MissingStagedFunction {
        function: Function,
        stage: CompileStage,
    },
    #[error("staged function {0:?} has no live specialization")]
    MissingSpecialization(StagedFunction),
    #[error("staged function {function:?} has {count} live specializations")]
    AmbiguousSpecialization {
        function: StagedFunction,
        count: usize,
    },
    #[error("function body fell through at {0:?}")]
    FunctionBodyFellThrough(Location),
    #[error("missing call target {target:?} at {location:?}")]
    MissingCallTarget { location: Location, target: Symbol },
    #[error("call result arity mismatch at {location:?}: expected {expected}, got {actual}")]
    CallResultArityMismatch {
        location: Location,
        expected: usize,
        actual: usize,
    },
    #[error("expected product value")]
    ExpectedProduct,
    #[error("product arity mismatch: expected {expected}, got {actual}")]
    ProductArityMismatch { expected: usize, actual: usize },
    #[error("indeterminate branch condition")]
    IndeterminateBranch,
    #[error("loop step overflow")]
    LoopStepOverflow,
    #[error("specialized function {function:?} has no body at stage {stage:?}")]
    MissingFunctionBody {
        function: SpecializedFunction,
        stage: CompileStage,
    },
    #[error("{0}")]
    Custom(&'static str),
}
