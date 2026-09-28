use crate::rewrite::drive_pass;
use crate::{CompileStage, Dialect, HasStageInfo, Pipeline, Quarantined, Rewriter, StageMeta};

use super::{
    StageDispatchMiss, StageDispatchRequiredError, StagePassError, SupportsStageDispatch,
    SupportsStageDispatchMut,
    helpers::{dispatch_optional_with, dispatch_required_with, map_required_miss_or_else},
    slot::StageStatus,
};

impl<S> Pipeline<S>
where
    S: StageMeta,
{
    /// Resolve `stage_id`, dispatch to the first matching dialect in
    /// `S::Languages`, and run `action`.
    ///
    /// Returns `Ok(None)` when `stage_id` does not exist, when its stage is
    /// unavailable because a pass holds or poisoned it, or when no dialect in
    /// `S::Languages` matches the concrete stage variant. Use
    /// [`Self::dispatch_stage_required`] to tell those apart.
    pub fn dispatch_stage<A, R, E>(
        &self,
        stage_id: CompileStage,
        action: &mut A,
    ) -> Result<Option<R>, E>
    where
        S: SupportsStageDispatch<A, R, E>,
    {
        dispatch_optional_with(
            self.stage_or_miss(stage_id),
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
            self.stage_or_miss(stage_id),
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
    /// Returns `Ok(None)` when `stage_id` does not exist, when its stage is
    /// unavailable because a pass holds or poisoned it, or when no dialect in
    /// `S::Languages` matches the concrete stage variant. Use
    /// [`Self::dispatch_stage_mut_required`] to tell those apart.
    pub fn dispatch_stage_mut<A, R, E>(
        &mut self,
        stage_id: CompileStage,
        action: &mut A,
    ) -> Result<Option<R>, E>
    where
        S: SupportsStageDispatchMut<A, R, E>,
    {
        dispatch_optional_with(
            self.stage_or_miss_mut(stage_id),
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
            self.stage_or_miss_mut(stage_id),
            stage_id,
            action,
            |stage, stage_id, action| {
                <S as SupportsStageDispatchMut<A, R, E>>::dispatch_stage_action_mut(
                    stage, stage_id, action,
                )
            },
        )
    }

    /// Run `pass` over the `L` stage info held at `stage_id`, in place.
    ///
    /// A pass needs exclusive ownership of what it rewrites, so the stage leaves
    /// its slot for the duration and comes back only if the pass succeeded *and*
    /// left the derived metadata agreeing with the IR.
    ///
    /// On failure, the slot is poisoned in place rather than emptied. It keeps
    /// its position, so every [`CompileStage`] issued so far stays valid, and
    /// it keeps the failure artifact, readable through [`Pipeline::quarantined`].
    ///
    /// There is no rollback: the edits a failed pass already made stay applied,
    /// which is why that stage can never be handed out again.
    pub fn run_pass<L, F, T, E>(
        &mut self,
        stage_id: CompileStage,
        pass: F,
    ) -> Result<T, StagePassError>
    where
        L: Dialect,
        E: std::error::Error + Send + Sync + 'static,
        F: FnOnce(&mut Rewriter<L>) -> Result<T, E>,
        S: HasStageInfo<L>,
    {
        let Some(slot) = self.slot_mut(stage_id) else {
            return Err(StagePassError::UnknownStage);
        };

        // A refused `take` leaves the slot untouched, so its state still says
        // which of the two reasons applies.
        let Some(mut stage) = slot.take() else {
            return Err(match slot.status() {
                StageStatus::Poisoned => StagePassError::StagePoisoned,
                _ => StagePassError::StageTransferred,
            });
        };

        let Some(stage_info) = stage.try_stage_info_mut() else {
            // The caller named a dialect this stage does not hold. Nothing ran,
            // so put it back rather than stranding the slot as transferred.
            slot.restore(stage);
            return Err(StagePassError::DialectMismatch);
        };

        // Borrow the stage info out of the stage and run the pass through it.
        let (events, result) = drive_pass(stage_info, pass);

        match result {
            Ok(output) => {
                slot.restore(stage);
                Ok(output)
            }
            Err(cause) => {
                slot.poison(Quarantined::from_stage::<L>(stage, cause, events));
                Err(StagePassError::PassFailed)
            }
        }
    }
}
