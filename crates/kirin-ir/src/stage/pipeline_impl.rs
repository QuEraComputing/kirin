use crate::{CompileStage, Pipeline, StageMeta};

use super::{
    StageDispatchMiss, StageDispatchRequiredError, SupportsStageDispatch, SupportsStageDispatchMut,
    helpers::{dispatch_optional_with, dispatch_required_with, map_required_miss_or_else},
};

impl<S> Pipeline<S>
where
    S: StageMeta,
{
    /// Resolve `stage_id`, dispatch to the first matching dialect in
    /// `S::Languages`, and run `action`.
    ///
    /// Returns `Ok(None)` when `stage_id` does not exist or no dialect in
    /// `S::Languages` matches the concrete stage variant.
    pub fn dispatch_stage<A, R, E>(
        &self,
        stage_id: CompileStage,
        action: &mut A,
    ) -> Result<Option<R>, E>
    where
        S: SupportsStageDispatch<A, R, E>,
    {
        dispatch_optional_with(
            self.stage(stage_id),
            stage_id,
            action,
            |stage, stage_id, action| {
                <S as SupportsStageDispatch<A, R, E>>::dispatch_stage_action(
                    stage, stage_id, action,
                )
            },
        )
    }

    /// Like [`Self::dispatch_stage`], but maps dispatch misses into `Err`
    /// using `on_miss`.
    pub fn dispatch_stage_or_else<A, R, E, F>(
        &self,
        stage_id: CompileStage,
        action: &mut A,
        on_miss: F,
    ) -> Result<R, E>
    where
        S: SupportsStageDispatch<A, R, E>,
        F: FnMut(StageDispatchMiss) -> E,
    {
        map_required_miss_or_else(self.dispatch_stage_required(stage_id, action), on_miss)
    }

    /// Like [`Self::dispatch_stage`], but converts dispatch misses into
    /// [`StageDispatchRequiredError::Miss`].
    pub fn dispatch_stage_required<A, R, E>(
        &self,
        stage_id: CompileStage,
        action: &mut A,
    ) -> Result<R, StageDispatchRequiredError<E>>
    where
        S: SupportsStageDispatch<A, R, E>,
    {
        dispatch_required_with(
            self.stage(stage_id),
            stage_id,
            action,
            |stage, stage_id, action| {
                <S as SupportsStageDispatch<A, R, E>>::dispatch_stage_action(
                    stage, stage_id, action,
                )
            },
        )
    }

    /// Mutable variant of [`Self::dispatch_stage`].
    ///
    /// Returns `Ok(None)` when `stage_id` does not exist or no dialect in
    /// `S::Languages` matches the concrete stage variant.
    pub fn dispatch_stage_mut<A, R, E>(
        &mut self,
        stage_id: CompileStage,
        action: &mut A,
    ) -> Result<Option<R>, E>
    where
        S: SupportsStageDispatchMut<A, R, E>,
    {
        dispatch_optional_with(
            self.stage_mut(stage_id),
            stage_id,
            action,
            |stage, stage_id, action| {
                <S as SupportsStageDispatchMut<A, R, E>>::dispatch_stage_action_mut(
                    stage, stage_id, action,
                )
            },
        )
    }

    /// Like [`Self::dispatch_stage_mut`], but maps dispatch misses into `Err`
    /// using `on_miss`.
    pub fn dispatch_stage_mut_or_else<A, R, E, F>(
        &mut self,
        stage_id: CompileStage,
        action: &mut A,
        on_miss: F,
    ) -> Result<R, E>
    where
        S: SupportsStageDispatchMut<A, R, E>,
        F: FnMut(StageDispatchMiss) -> E,
    {
        map_required_miss_or_else(self.dispatch_stage_mut_required(stage_id, action), on_miss)
    }

    /// Like [`Self::dispatch_stage_mut`], but converts dispatch misses into
    /// [`StageDispatchRequiredError::Miss`].
    pub fn dispatch_stage_mut_required<A, R, E>(
        &mut self,
        stage_id: CompileStage,
        action: &mut A,
    ) -> Result<R, StageDispatchRequiredError<E>>
    where
        S: SupportsStageDispatchMut<A, R, E>,
    {
        dispatch_required_with(
            self.stage_mut(stage_id),
            stage_id,
            action,
            |stage, stage_id, action| {
                <S as SupportsStageDispatchMut<A, R, E>>::dispatch_stage_action_mut(
                    stage, stage_id, action,
                )
            },
        )
    }
}
