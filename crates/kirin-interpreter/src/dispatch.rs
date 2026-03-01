use kirin_ir::{CompileStage, Pipeline, StageDispatchMiss, StageMeta, SupportsStageDispatch};

use crate::InterpreterError;

/// Convert a stage-dispatch miss into the framework error model.
pub(crate) fn map_dispatch_miss<E: From<InterpreterError>>(
    stage_id: CompileStage,
    miss: StageDispatchMiss,
) -> E {
    match miss {
        StageDispatchMiss::MissingStage => InterpreterError::MissingStage { stage: stage_id },
        StageDispatchMiss::MissingDialect => {
            InterpreterError::MissingStageDialect { stage: stage_id }
        }
    }
    .into()
}

/// Dispatch a runtime action against `stage_id` using `pipeline`.
pub(crate) fn dispatch_in_pipeline<S, A, R, E>(
    pipeline: &Pipeline<S>,
    stage_id: CompileStage,
    action: &mut A,
) -> Result<R, E>
where
    S: StageMeta + SupportsStageDispatch<A, R, E>,
    E: From<InterpreterError>,
{
    pipeline.dispatch_stage_or_else(stage_id, action, |miss| map_dispatch_miss(stage_id, miss))
}
