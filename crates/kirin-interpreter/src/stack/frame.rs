use kirin_ir::{
    Block, CompileStage, Dialect, HasStageInfo, Pipeline, ResultValue, SSAValue, StageInfo,
    StageMeta, Statement, SupportsStageDispatch,
};

use super::{
    DynFrameDispatch, FrameDispatchAction, StackFrame, StackFrameExtra, StackInterpreter,
    StageDispatchTable,
};
use crate::{
    ConcreteExt, Continuation, Frame, Interpretable, Interpreter, InterpreterError, StageAccess,
    ValueStore,
};

impl<'ir, V, S, E, G> StackInterpreter<'ir, V, S, E, G>
where
    E: From<InterpreterError>,
    S: StageMeta,
{
    pub(super) fn build_dispatch_table(
        pipeline: &'ir Pipeline<S>,
    ) -> StageDispatchTable<'ir, V, S, E, G>
    where
        V: Clone + 'ir,
        E: 'ir,
        S: 'ir
            + SupportsStageDispatch<
                FrameDispatchAction<'ir, V, S, E, G>,
                DynFrameDispatch<'ir, V, S, E, G>,
                E,
            >,
        G: 'ir,
    {
        let mut by_stage = Vec::with_capacity(pipeline.stages().len());
        for stage in pipeline.stages() {
            let dispatch = stage.stage_id().and_then(|stage_id| {
                Self::resolve_dispatch_for_stage_in_pipeline(pipeline, stage_id).ok()
            });
            by_stage.push(dispatch);
        }
        StageDispatchTable { by_stage }
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
        V: Clone + 'ir,
        E: 'ir,
        S: 'ir
            + SupportsStageDispatch<
                FrameDispatchAction<'ir, V, S, E, G>,
                DynFrameDispatch<'ir, V, S, E, G>,
                E,
            >,
        G: 'ir,
    {
        let dispatch = self.resolve_dispatch_for_stage(frame.stage())?;
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
    V: Clone + 'ir,
    E: From<InterpreterError> + 'ir,
    S: StageMeta + 'ir,
    G: 'ir,
{
    type Value = V;
    type Error = E;

    fn read(&self, value: SSAValue) -> Result<V, E> {
        self.frames.read(value).cloned()
    }

    fn write(&mut self, result: ResultValue, value: V) -> Result<(), E> {
        self.frames.write(result, value)
    }

    fn write_ssa(&mut self, ssa: SSAValue, value: V) -> Result<(), E> {
        self.frames.write_ssa(ssa, value)
    }
}

impl<'ir, V, S, E, G> StageAccess<'ir> for StackInterpreter<'ir, V, S, E, G>
where
    V: Clone + 'ir,
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

impl<'ir, V, S, E, G> Interpreter<'ir> for StackInterpreter<'ir, V, S, E, G>
where
    V: Clone + 'ir,
    E: From<InterpreterError> + 'ir,
    S: StageMeta + 'ir,
    G: 'ir,
{
    type Ext = ConcreteExt;

    fn eval_block<L: Dialect>(
        &mut self,
        stage: &'ir StageInfo<L>,
        block: Block,
    ) -> Result<Continuation<V, ConcreteExt>, E>
    where
        S: HasStageInfo<L>,
        L: Interpretable<'ir, Self, L>,
    {
        let saved_cursor = self.current_cursor()?;
        let first = block.first_statement(stage);
        self.set_current_cursor(first)?;
        let v = self.run_nested_calls_cached(|_interp, is_yield| is_yield)?;
        self.set_current_cursor(saved_cursor)?;
        Ok(Continuation::Yield(v))
    }
}
