/// Why stage dispatch returned no action result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StageDispatchMiss {
    /// The requested stage ID is not present in the pipeline.
    MissingStage,
    /// The position exists, but its stage cannot be reached: a pass either has
    /// it out on loan or failed and poisoned the position.
    StageUnavailable,
    /// The stage exists but no dialect in `S::Languages` matched it.
    MissingDialect,
}

/// Error for required dispatch helpers.
#[derive(Debug, PartialEq, Eq)]
pub enum StageDispatchRequiredError<E> {
    /// Action-specific failure produced by `StageAction`/`StageActionMut`.
    Action(E),
    /// Dispatch miss describing why no stage action could run.
    Miss(StageDispatchMiss),
}

/// Why [`Pipeline::run_pass`](crate::Pipeline::run_pass) did not hand back a
/// pass result.
///
/// Only [`PassFailed`](StagePassError::PassFailed) leaves the pipeline changed.
/// Under every other variant the stage is exactly as it was, so the caller may
/// fix the call and try again.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StagePassError {
    /// No position in the pipeline has this [`CompileStage`](crate::CompileStage).
    UnknownStage,
    /// The position exists, but a pass took its stage and never gave it back.
    /// Likely, an unwind escaped between the two.
    StageTransferred,
    /// The position exists, but an *earlier* pass failed here.
    StagePoisoned,
    /// The stage at this position holds no stage info for the dialect the pass
    /// was written against, so the caller named the wrong one. Nothing ran and
    /// the stage is still usable.
    DialectMismatch,
    /// This pass failed, so the position is now poisoned and that stage must be
    /// abandoned. The cause, event log, and report are available from
    /// [`Pipeline::quarantined`](crate::Pipeline::quarantined).
    PassFailed,
}

impl std::fmt::Display for StagePassError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StagePassError::UnknownStage => {
                write!(f, "no stage at that position in the pipeline")
            }
            StagePassError::StageTransferred => {
                write!(f, "the stage was lent to a pass that never returned it")
            }
            StagePassError::StagePoisoned => {
                write!(f, "an earlier pass failed at this position")
            }
            StagePassError::DialectMismatch => {
                write!(f, "the stage holds no stage info for the pass's dialect")
            }
            StagePassError::PassFailed => {
                write!(f, "the pass failed; this position is now poisoned")
            }
        }
    }
}

impl std::error::Error for StagePassError {}
