use crate::language::Dialect;
use crate::node::function::CompileStage;
use crate::stage::action::{StageAction, StageActionMut};
use crate::stage::meta::{HasStageInfo, StageMeta};

/// Recursive dispatcher over `S::Languages` for immutable stage access.
///
/// This trait is implemented for `()` and nested tuples `(L, Tail)` and is
/// intended to be used by [`crate::Pipeline::dispatch_stage`].
pub trait StageDispatch<S, A, R, E>
where
    S: StageMeta,
{
    fn dispatch(stage: &S, stage_id: CompileStage, action: &mut A) -> Result<Option<R>, E>;
}

impl<S, A, R, E> StageDispatch<S, A, R, E> for ()
where
    S: StageMeta,
{
    fn dispatch(_stage: &S, _stage_id: CompileStage, _action: &mut A) -> Result<Option<R>, E> {
        Ok(None)
    }
}

impl<S, L, Tail, A, R, E> StageDispatch<S, A, R, E> for (L, Tail)
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
    A: StageAction<S, L, Output = R, Error = E>,
    Tail: StageDispatch<S, A, R, E>,
{
    fn dispatch(stage: &S, stage_id: CompileStage, action: &mut A) -> Result<Option<R>, E> {
        if let Some(stage_info) = <S as HasStageInfo<L>>::try_stage_info(stage) {
            return action.run(stage_id, stage_info).map(Some);
        }
        <Tail as StageDispatch<S, A, R, E>>::dispatch(stage, stage_id, action)
    }
}

/// Recursive dispatcher over `S::Languages` for mutable stage access.
///
/// This trait is implemented for `()` and nested tuples `(L, Tail)` and is
/// intended to be used by [`crate::Pipeline::dispatch_stage_mut`].
pub trait StageDispatchMut<S, A, R, E>
where
    S: StageMeta,
{
    fn dispatch(stage: &mut S, stage_id: CompileStage, action: &mut A) -> Result<Option<R>, E>;
}

/// Marker trait for stage containers that support immutable dispatch of `A`.
pub trait SupportsStageDispatch<A, R, E>: StageMeta {
    fn dispatch_stage_action(
        stage: &Self,
        stage_id: CompileStage,
        action: &mut A,
    ) -> Result<Option<R>, E>;
}

impl<S, A, R, E> SupportsStageDispatch<A, R, E> for S
where
    S: StageMeta,
    S::Languages: StageDispatch<S, A, R, E>,
{
    fn dispatch_stage_action(
        stage: &Self,
        stage_id: CompileStage,
        action: &mut A,
    ) -> Result<Option<R>, E> {
        <S::Languages as StageDispatch<S, A, R, E>>::dispatch(stage, stage_id, action)
    }
}

/// Marker trait for stage containers that support mutable dispatch of `A`.
pub trait SupportsStageDispatchMut<A, R, E>: StageMeta {
    fn dispatch_stage_action_mut(
        stage: &mut Self,
        stage_id: CompileStage,
        action: &mut A,
    ) -> Result<Option<R>, E>;
}

impl<S, A, R, E> SupportsStageDispatchMut<A, R, E> for S
where
    S: StageMeta,
    S::Languages: StageDispatchMut<S, A, R, E>,
{
    fn dispatch_stage_action_mut(
        stage: &mut Self,
        stage_id: CompileStage,
        action: &mut A,
    ) -> Result<Option<R>, E> {
        <S::Languages as StageDispatchMut<S, A, R, E>>::dispatch(stage, stage_id, action)
    }
}

impl<S, A, R, E> StageDispatchMut<S, A, R, E> for ()
where
    S: StageMeta,
{
    fn dispatch(_stage: &mut S, _stage_id: CompileStage, _action: &mut A) -> Result<Option<R>, E> {
        Ok(None)
    }
}

impl<S, L, Tail, A, R, E> StageDispatchMut<S, A, R, E> for (L, Tail)
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
    A: StageActionMut<S, L, Output = R, Error = E>,
    Tail: StageDispatchMut<S, A, R, E>,
{
    fn dispatch(stage: &mut S, stage_id: CompileStage, action: &mut A) -> Result<Option<R>, E> {
        if let Some(stage_info) = <S as HasStageInfo<L>>::try_stage_info_mut(stage) {
            return action.run(stage_id, stage_info).map(Some);
        }
        <Tail as StageDispatchMut<S, A, R, E>>::dispatch(stage, stage_id, action)
    }
}
