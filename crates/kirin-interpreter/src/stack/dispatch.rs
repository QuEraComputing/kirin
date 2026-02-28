use std::marker::PhantomData;

use kirin_ir::{
    CompileStage, Dialect, HasStageInfo, SpecializedFunction, StageAction, StageInfo, StageMeta,
    SupportsStageDispatch,
};

use super::StackInterpreter;
use crate::{ConcreteContinuation, EvalCall, Interpretable, Interpreter, InterpreterError};

pub(super) type DynStepFn<'ir, V, S, E, G> =
    fn(&mut StackInterpreter<'ir, V, S, E, G>) -> Result<ConcreteContinuation<V>, E>;
pub(super) type DynAdvanceFn<'ir, V, S, E, G> =
    fn(&mut StackInterpreter<'ir, V, S, E, G>, &ConcreteContinuation<V>) -> Result<(), E>;

#[doc(hidden)]
pub struct DynFrameDispatch<'ir, V, S, E, G>
where
    S: StageMeta,
{
    pub(super) step: DynStepFn<'ir, V, S, E, G>,
    pub(super) advance: DynAdvanceFn<'ir, V, S, E, G>,
}

impl<'ir, V, S, E, G> Copy for DynFrameDispatch<'ir, V, S, E, G> where S: StageMeta {}

impl<'ir, V, S, E, G> Clone for DynFrameDispatch<'ir, V, S, E, G>
where
    S: StageMeta,
{
    fn clone(&self) -> Self {
        *self
    }
}

#[doc(hidden)]
pub struct CallDynAction<'a, 'ir, V, S, E, G>
where
    S: StageMeta,
{
    pub(super) interp: &'a mut StackInterpreter<'ir, V, S, E, G>,
    pub(super) callee: SpecializedFunction,
    pub(super) args: &'a [V],
}

impl<'a, 'ir, V, S, E, G, L> StageAction<S, L> for CallDynAction<'a, 'ir, V, S, E, G>
where
    V: Clone + 'ir,
    E: From<InterpreterError> + 'ir,
    S: StageMeta + HasStageInfo<L> + 'ir,
    G: 'ir,
    L: Dialect
        + Interpretable<'ir, StackInterpreter<'ir, V, S, E, G>, L>
        + EvalCall<'ir, StackInterpreter<'ir, V, S, E, G>, L, Result = V>
        + 'ir,
{
    type Output = V;
    type Error = E;

    fn run(
        &mut self,
        stage_id: CompileStage,
        _stage: &StageInfo<L>,
    ) -> Result<Self::Output, Self::Error> {
        let stage = self.interp.resolve_stage_info::<L>(stage_id)?;
        self.interp.with_stage(stage).call(self.callee, self.args)
    }
}

fn dyn_step_for_lang<'ir, V, S, E, G, L>(
    interp: &mut StackInterpreter<'ir, V, S, E, G>,
) -> Result<ConcreteContinuation<V>, E>
where
    V: Clone + 'ir,
    E: From<InterpreterError> + 'ir,
    S: StageMeta + HasStageInfo<L> + 'ir,
    G: 'ir,
    L: Dialect + Interpretable<'ir, StackInterpreter<'ir, V, S, E, G>, L> + 'ir,
{
    interp.in_stage::<L>().step()
}

fn dyn_advance_for_lang<'ir, V, S, E, G, L>(
    interp: &mut StackInterpreter<'ir, V, S, E, G>,
    control: &ConcreteContinuation<V>,
) -> Result<(), E>
where
    V: Clone + 'ir,
    E: From<InterpreterError> + 'ir,
    S: StageMeta + HasStageInfo<L> + 'ir,
    G: 'ir,
    L: Dialect + Interpretable<'ir, StackInterpreter<'ir, V, S, E, G>, L> + 'ir,
{
    let stage_id = interp.call_stack.current()?.stage();
    let stage = interp.resolve_stage_info::<L>(stage_id)?;
    interp.advance_frame_with_stage::<L>(stage, control)
}

#[doc(hidden)]
pub struct FrameDispatchAction<'ir, V, S, E, G>
where
    S: StageMeta,
{
    marker: PhantomData<(&'ir (), V, S, E, G)>,
}

impl<'ir, V, S, E, G, L> StageAction<S, L> for FrameDispatchAction<'ir, V, S, E, G>
where
    V: Clone + 'ir,
    E: From<InterpreterError> + 'ir,
    S: StageMeta + HasStageInfo<L> + 'ir,
    G: 'ir,
    L: Dialect + Interpretable<'ir, StackInterpreter<'ir, V, S, E, G>, L> + 'ir,
{
    type Output = DynFrameDispatch<'ir, V, S, E, G>;
    type Error = E;

    fn run(
        &mut self,
        _stage_id: CompileStage,
        _stage: &StageInfo<L>,
    ) -> Result<Self::Output, Self::Error> {
        Ok(DynFrameDispatch {
            step: dyn_step_for_lang::<V, S, E, G, L>,
            advance: dyn_advance_for_lang::<V, S, E, G, L>,
        })
    }
}

#[doc(hidden)]
pub struct PushCallFrameDynAction<'a, 'ir, V, S, E, G>
where
    S: StageMeta,
{
    interp: &'a mut StackInterpreter<'ir, V, S, E, G>,
    callee: SpecializedFunction,
    args: &'a [V],
}

impl<'a, 'ir, V, S, E, G, L> StageAction<S, L> for PushCallFrameDynAction<'a, 'ir, V, S, E, G>
where
    V: Clone + 'ir,
    E: From<InterpreterError> + 'ir,
    S: StageMeta + HasStageInfo<L> + 'ir,
    S: SupportsStageDispatch<
            FrameDispatchAction<'ir, V, S, E, G>,
            DynFrameDispatch<'ir, V, S, E, G>,
            E,
        >,
    G: 'ir,
    L: Dialect + Interpretable<'ir, StackInterpreter<'ir, V, S, E, G>, L> + 'ir,
{
    type Output = ();
    type Error = E;

    fn run(
        &mut self,
        stage_id: CompileStage,
        _stage: &StageInfo<L>,
    ) -> Result<Self::Output, Self::Error> {
        let stage = self.interp.resolve_stage_info::<L>(stage_id)?;
        self.interp
            .with_stage(stage)
            .push_call_frame(self.callee, self.args)
    }
}

impl<'a, 'ir, V, S, E, G> PushCallFrameDynAction<'a, 'ir, V, S, E, G>
where
    S: StageMeta,
{
    pub(super) fn new(
        interp: &'a mut StackInterpreter<'ir, V, S, E, G>,
        callee: SpecializedFunction,
        args: &'a [V],
    ) -> Self {
        Self {
            interp,
            callee,
            args,
        }
    }
}

impl<'ir, V, S, E, G> FrameDispatchAction<'ir, V, S, E, G>
where
    S: StageMeta,
{
    pub(super) fn new() -> Self {
        Self {
            marker: PhantomData,
        }
    }
}
