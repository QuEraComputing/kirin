use std::convert::Infallible;

use kirin_ir::{Block, CompileStage, Function, SSAValue, StagedFunction, Statement, Symbol};
use thiserror::Error;

use crate::EnvIndex;

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
    #[error("missing block info for block {0:?}")]
    MissingBlock(Block),
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
    #[error("missing call target {0:?}")]
    MissingCallTarget(Symbol),
    #[error("region has no entry block")]
    EmptyRegion,
    #[error("block {0:?} fell through without a terminator effect")]
    BlockFellThrough(Block),
    #[error("function body fell through without returning")]
    FunctionBodyFellThrough,
    #[error("yield outside of an enclosing scope at {0:?}")]
    UnexpectedYield(Statement),
    #[error("statement {0:?} is not callable")]
    NotCallable(Statement),
    #[error("block argument arity mismatch at {block:?}: expected {expected}, got {actual}")]
    BlockArityMismatch {
        block: Block,
        expected: usize,
        actual: usize,
    },
    #[error("product arity mismatch: expected {expected}, got {actual}")]
    ProductArityMismatch { expected: usize, actual: usize },
    #[error("indeterminate branch condition")]
    IndeterminateBranch,
    #[error("loop step overflow")]
    LoopStepOverflow,
    #[error("fixpoint iteration limit exceeded")]
    FixpointDiverged,
    #[error("missing stage named {0:?}")]
    MissingStageName(String),
    #[error("missing function named {0:?}")]
    MissingFunctionName(String),
    #[error("expected a single function return value, got {0}")]
    ExpectedSingleReturn(usize),
    #[error("{0}")]
    Custom(&'static str),
}

impl From<Infallible> for InterpreterError {
    fn from(error: Infallible) -> Self {
        match error {}
    }
}
