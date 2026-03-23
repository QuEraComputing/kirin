use std::marker::PhantomData;

use kirin_ir::{
    CompileStage, Dialect, HasStageInfo, SpecializedFunction, StageAction, StageInfo, StageMeta,
};
use smallvec::SmallVec;

use super::StackInterpreter;
use crate::{
    CallSemantics, ConcreteExt, Continuation, Interpretable, InterpreterError, StageAccess,
};

pub(super) type DynStepFn<'ir, V, S, E, G> =
    fn(&mut StackInterpreter<'ir, V, S, E, G>) -> Result<Continuation<V, ConcreteExt>, E>;
pub(super) type DynAdvanceFn<'ir, V, S, E, G> =
    fn(&mut StackInterpreter<'ir, V, S, E, G>, &Continuation<V, ConcreteExt>) -> Result<(), E>;
pub(super) type DynPushCallFrameFn<'ir, V, S, E, G> = fn(
    &mut StackInterpreter<'ir, V, S, E, G>,
    CompileStage,
    SpecializedFunction,
    &[V],
) -> Result<(), E>;

#[doc(hidden)]
pub struct DynFrameDispatch<'ir, V, S, E, G>
where
    S: StageMeta,
{
    pub(super) step: DynStepFn<'ir, V, S, E, G>,
    pub(super) advance: DynAdvanceFn<'ir, V, S, E, G>,
    pub(super) push_call_frame: DynPushCallFrameFn<'ir, V, S, E, G>,
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
        + Interpretable<'ir, StackInterpreter<'ir, V, S, E, G>>
        + CallSemantics<'ir, StackInterpreter<'ir, V, S, E, G>, Result = SmallVec<[V; 1]>>
        + 'ir,
{
    type Output = SmallVec<[V; 1]>;
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
) -> Result<Continuation<V, ConcreteExt>, E>
where
    V: Clone + 'ir,
    E: From<InterpreterError> + 'ir,
    S: StageMeta + HasStageInfo<L> + 'ir,
    G: 'ir,
    L: Dialect + Interpretable<'ir, StackInterpreter<'ir, V, S, E, G>> + 'ir,
{
    interp.in_stage::<L>().step()
}

fn dyn_push_call_frame_for_lang<'ir, V, S, E, G, L>(
    interp: &mut StackInterpreter<'ir, V, S, E, G>,
    stage_id: CompileStage,
    callee: SpecializedFunction,
    args: &[V],
) -> Result<(), E>
where
    V: Clone + 'ir,
    E: From<InterpreterError> + 'ir,
    S: StageMeta + HasStageInfo<L> + 'ir,
    G: 'ir,
    L: Dialect + Interpretable<'ir, StackInterpreter<'ir, V, S, E, G>> + 'ir,
{
    let stage = interp.resolve_stage_info::<L>(stage_id)?;
    interp.push_call_frame_with_stage::<L>(callee, stage, args)
}

fn dyn_advance_for_lang<'ir, V, S, E, G, L>(
    interp: &mut StackInterpreter<'ir, V, S, E, G>,
    control: &Continuation<V, ConcreteExt>,
) -> Result<(), E>
where
    V: Clone + 'ir,
    E: From<InterpreterError> + 'ir,
    S: StageMeta + HasStageInfo<L> + 'ir,
    G: 'ir,
    L: Dialect + Interpretable<'ir, StackInterpreter<'ir, V, S, E, G>> + 'ir,
{
    let stage_id = interp.frames.current()?.stage();
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
    L: Dialect + Interpretable<'ir, StackInterpreter<'ir, V, S, E, G>> + 'ir,
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
            push_call_frame: dyn_push_call_frame_for_lang::<V, S, E, G, L>,
        })
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
