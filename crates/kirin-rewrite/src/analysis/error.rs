use std::any::type_name;
use std::error::Error;
use std::fmt;

use kirin_ir::StagePassError;

use super::AnalysisKind;

/// Why an analysis result could not be computed.
#[derive(Debug)]
pub enum AnalysisError {
    /// The analysis itself failed.
    Failed {
        analysis: &'static str,
        source: Box<dyn Error + Send + Sync>,
    },
    /// Computing `analysis` for a key needed `analysis` for the same key
    /// again. The same analysis for a different key is not a cycle.
    Cycle { analysis: &'static str },
    /// A pass required `analysis`, but it was never registered.
    Unregistered { analysis: &'static str },
    /// The key names a stage the program does not hold, or one a failed pass
    /// quarantined. `run` returns it.
    UnknownStage,
}

impl AnalysisError {
    /// Analysis `A` failed with `source`.
    pub fn failed<A: AnalysisKind>(source: impl Into<Box<dyn Error + Send + Sync>>) -> Self {
        AnalysisError::Failed {
            analysis: type_name::<A>(),
            source: source.into(),
        }
    }
}

impl fmt::Display for AnalysisError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AnalysisError::Failed { analysis, source } => {
                write!(f, "analysis `{analysis}` failed: {source}")
            }
            AnalysisError::Cycle { analysis } => {
                write!(
                    f,
                    "analysis `{analysis}` depends on itself for the same key"
                )
            }
            AnalysisError::Unregistered { analysis } => {
                write!(f, "analysis `{analysis}` was required but never registered")
            }
            AnalysisError::UnknownStage => write!(f, "no usable stage at that position"),
        }
    }
}

impl Error for AnalysisError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            AnalysisError::Failed { source, .. } => Some(source.as_ref()),
            _ => None,
        }
    }
}

/// Why [`Database::run_pass`](super::Database::run_pass) failed.
#[derive(Debug)]
pub enum RunPassError {
    /// A required analysis could not be computed. The pass did not run and
    /// the stage is untouched.
    RequirementUnsatisfied(AnalysisError),
    /// [`Program::run_pass`](kirin_ir::Program::run_pass) returned an
    /// error: the stage could not be run, or the pass failed.
    PassFailedOrNotRun(StagePassError),
}

impl fmt::Display for RunPassError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RunPassError::RequirementUnsatisfied(error) => {
                write!(f, "a requirement could not be satisfied: {error}")
            }
            RunPassError::PassFailedOrNotRun(error) => {
                write!(f, "running the pass on the stage failed: {error}")
            }
        }
    }
}

impl Error for RunPassError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            RunPassError::RequirementUnsatisfied(error) => Some(error),
            RunPassError::PassFailedOrNotRun(error) => Some(error),
        }
    }
}
