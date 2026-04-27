use kirin_ir::{CompileStage, SSAValue};
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
}
