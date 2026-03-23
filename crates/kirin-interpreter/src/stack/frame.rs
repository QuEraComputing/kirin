use kirin_ir::{
    Block, CompileStage, Dialect, HasStageInfo, Pipeline, SSAValue, StageInfo, StageMeta,
    Statement, SupportsStageDispatch,
};

use super::{DynFrameDispatch, FrameDispatchAction, StackFrame, StackFrameExtra, StackInterpreter};
use crate::dispatch::DispatchCache;
use crate::{
    BlockEvaluator, ConcreteExt, Continuation, Frame, Interpretable, InterpreterError, StageAccess,
    ValueStore,
};

impl<'ir, V, S, E, G> StackInterpreter<'ir, V, S, E, G>
where
    E: From<InterpreterError>,
    S: StageMeta,
{
    pub(super) fn build_dispatch_table(
        pipeline: &'ir Pipeline<S>,
    ) -> DispatchCache<DynFrameDispatch<'ir, V, S, E, G>>
    where
        V: Clone + crate::ProductValue + 'ir,
        E: 'ir,
        S: 'ir
            + SupportsStageDispatch<
                FrameDispatchAction<'ir, V, S, E, G>,
                DynFrameDispatch<'ir, V, S, E, G>,
                E,
            >,
        G: 'ir,
    {
        DispatchCache::build(pipeline, |_pipeline, stage_id| {
            Self::resolve_dispatch_for_stage_in_pipeline(pipeline, stage_id)
        })
    }

    pub(crate) fn current_cursor(&self) -> Result<Option<Statement>, E> {
        Ok(self.frames.current()?.extra().cursor)
    }

    pub(crate) fn set_current_cursor(&mut self, cursor: Option<Statement>) -> Result<(), E> {
        self.frames.current_mut()?.extra_mut().cursor = cursor;
        Ok(())
    }

    pub(crate) fn frame_depth(&self) -> usize {
        self.frames.depth()
    }

    pub(super) fn public_frame_to_internal(
        frame: Frame<V, Option<Statement>>,
        dispatch: DynFrameDispatch<'ir, V, S, E, G>,
    ) -> StackFrame<'ir, V, S, E, G> {
        let (callee, stage, values, cursor) = frame.into_parts();
        Frame::with_values(callee, stage, values, StackFrameExtra { cursor, dispatch })
    }

    fn internal_frame_to_public(frame: StackFrame<'ir, V, S, E, G>) -> Frame<V, Option<Statement>> {
        let (callee, stage, values, extra) = frame.into_parts();
        Frame::with_values(callee, stage, values, extra.cursor)
    }

    /// Push a call frame and eagerly resolve per-frame dynamic dispatch from
    /// `frame.stage()`. Fails atomically when depth or stage dispatch checks fail.
    pub fn push_frame(&mut self, frame: Frame<V, Option<Statement>>) -> Result<(), E>
    where
        V: Clone + crate::ProductValue + 'ir,
        E: 'ir,
        S: 'ir,
        G: 'ir,
    {
        let dispatch = self.lookup_dispatch(frame.stage())?;
        let internal = Self::public_frame_to_internal(frame, dispatch);
        self.frames.push(internal)?;
        Ok(())
    }
}

impl<'ir, V, S, E, G> StackInterpreter<'ir, V, S, E, G>
where
    E: From<InterpreterError>,
    S: StageMeta,
{
    /// Pop the current call frame and its paired dynamic dispatch entry.
    pub fn pop_frame(&mut self) -> Result<Frame<V, Option<Statement>>, E> {
        let frame = self.frames.pop()?;
        Ok(Self::internal_frame_to_public(frame))
    }
}

impl<'ir, V, S, E, G> ValueStore for StackInterpreter<'ir, V, S, E, G>
where
    V: Clone + crate::ProductValue + 'ir,
    E: From<InterpreterError> + 'ir,
    S: StageMeta + 'ir,
    G: 'ir,
{
    type Value = V;
    type Error = E;

    fn read(&self, value: SSAValue) -> Result<V, E> {
        self.frames.read(value).cloned()
    }

    fn write(&mut self, target: SSAValue, value: V) -> Result<(), E> {
        self.frames.write_ssa(target, value)
    }
}

impl<'ir, V, S, E, G> StageAccess<'ir> for StackInterpreter<'ir, V, S, E, G>
where
    V: Clone + crate::ProductValue + 'ir,
    E: From<InterpreterError> + 'ir,
    S: StageMeta + 'ir,
    G: 'ir,
{
    type StageInfo = S;

    fn pipeline(&self) -> &'ir Pipeline<S> {
        self.pipeline
    }

    fn active_stage(&self) -> CompileStage {
        self.frames.active_stage_or(self.root_stage)
    }
}

impl<'ir, V, S, E, G> BlockEvaluator<'ir> for StackInterpreter<'ir, V, S, E, G>
where
    V: Clone + crate::ProductValue + 'ir,
    E: From<InterpreterError> + 'ir,
    S: StageMeta + 'ir,
    G: 'ir,
{
    type Ext = ConcreteExt;

    #[allow(clippy::multiple_bound_locations)] // L: Dialect on method + where clause
    fn eval_block<L: Dialect>(
        &mut self,
        stage: &'ir StageInfo<L>,
        block: Block,
    ) -> Result<Continuation<V, ConcreteExt>, E>
    where
        S: HasStageInfo<L>,
        L: Interpretable<'ir, Self>,
    {
        let saved_cursor = self.current_cursor()?;
        let first = block.first_statement(stage);
        self.set_current_cursor(first)?;
        let v = self.run_nested_calls(|_interp, is_yield| is_yield)?;
        self.set_current_cursor(saved_cursor)?;
        Ok(Continuation::Yield(v))
    }
}
