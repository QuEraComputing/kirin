use crate::CompileStage;

use super::{StageDispatchMiss, StageDispatchRequiredError};

pub(super) fn dispatch_optional_with<StageRef, A, R, E, F>(
    stage: Option<StageRef>,
    stage_id: CompileStage,
    action: &mut A,
    dispatch: F,
) -> Result<Option<R>, E>
where
    F: FnOnce(StageRef, CompileStage, &mut A) -> Result<Option<R>, E>,
{
    let Some(stage) = stage else {
        return Ok(None);
    };
    dispatch(stage, stage_id, action)
}

pub(super) fn dispatch_required_with<StageRef, A, R, E, F>(
    stage: Option<StageRef>,
    stage_id: CompileStage,
    action: &mut A,
    dispatch: F,
) -> Result<R, StageDispatchRequiredError<E>>
where
    F: FnOnce(StageRef, CompileStage, &mut A) -> Result<Option<R>, E>,
{
    let Some(stage) = stage else {
        return Err(StageDispatchRequiredError::Miss(
            StageDispatchMiss::MissingStage,
        ));
    };
    let Some(result) =
        dispatch(stage, stage_id, action).map_err(StageDispatchRequiredError::Action)?
    else {
        return Err(StageDispatchRequiredError::Miss(
            StageDispatchMiss::MissingDialect,
        ));
    };
    Ok(result)
}

pub(super) fn map_required_miss_or_else<R, E, F>(
    result: Result<R, StageDispatchRequiredError<E>>,
    mut on_miss: F,
) -> Result<R, E>
where
    F: FnMut(StageDispatchMiss) -> E,
{
    result.map_err(|error| match error {
        StageDispatchRequiredError::Action(error) => error,
        StageDispatchRequiredError::Miss(miss) => on_miss(miss),
    })
}
