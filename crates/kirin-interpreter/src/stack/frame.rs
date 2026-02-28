use kirin_ir::{
    CompileStage, Pipeline, ResultValue, SSAValue, StageMeta, Statement, SupportsStageDispatch,
};

use super::{
    DynFrameDispatch, FrameDispatchAction, StackFrame, StackFrameExtra, StackInterpreter,
    StageDispatchTable,
};
use crate::{ConcreteExt, Frame, Interpreter, InterpreterError};

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

    fn current_frame_ref(&self) -> Result<&StackFrame<'ir, V, S, E, G>, E> {
        self.call_stack
            .last()
            .ok_or_else(|| InterpreterError::NoFrame.into())
    }

    fn current_frame_mut_ref(&mut self) -> Result<&mut StackFrame<'ir, V, S, E, G>, E> {
        self.call_stack
            .last_mut()
            .ok_or_else(|| InterpreterError::NoFrame.into())
    }

    pub(crate) fn current_cursor(&self) -> Result<Option<Statement>, E> {
        Ok(self.current_frame_ref()?.extra().cursor)
    }

    pub(crate) fn set_current_cursor(&mut self, cursor: Option<Statement>) -> Result<(), E> {
        self.current_frame_mut_ref()?.extra_mut().cursor = cursor;
        Ok(())
    }

    fn active_stage_from_frames(&self) -> CompileStage {
        self.call_stack
            .last()
            .map(Frame::stage)
            .unwrap_or(self.root_stage)
    }

    fn read_ref_from_current_frame(&self, value: SSAValue) -> Result<&V, E> {
        let frame = self.current_frame_ref()?;
        frame
            .read(value)
            .ok_or_else(|| InterpreterError::UnboundValue(value).into())
    }

    fn write_to_current_frame(&mut self, result: ResultValue, value: V) -> Result<(), E> {
        self.current_frame_mut_ref()?.write(result, value);
        Ok(())
    }

    fn write_ssa_to_current_frame(&mut self, ssa: SSAValue, value: V) -> Result<(), E> {
        self.current_frame_mut_ref()?.write_ssa(ssa, value);
        Ok(())
    }

    pub(crate) fn frame_depth(&self) -> usize {
        self.call_stack.len()
    }

    pub(super) fn current_frame_stage(&self) -> Result<CompileStage, E> {
        Ok(self.current_frame_ref()?.stage())
    }

    pub(super) fn current_frame_dispatch(&self) -> Result<DynFrameDispatch<'ir, V, S, E, G>, E> {
        Ok(self.current_frame_ref()?.extra().dispatch)
    }

    fn public_frame_to_internal(
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
        if let Some(max) = self.max_depth {
            if self.call_stack.len() >= max {
                return Err(InterpreterError::MaxDepthExceeded.into());
            }
        }
        let dispatch = self.resolve_dispatch_for_stage(frame.stage())?;
        let internal = Self::public_frame_to_internal(frame, dispatch);
        self.call_stack.push(internal);
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
        let frame = self
            .call_stack
            .pop()
            .ok_or_else(|| InterpreterError::NoFrame.into())?;
        Ok(Self::internal_frame_to_public(frame))
    }
}

impl<'ir, V, S, E, G> Interpreter<'ir> for StackInterpreter<'ir, V, S, E, G>
where
    V: 'ir,
    E: From<InterpreterError> + 'ir,
    S: StageMeta + 'ir,
    G: 'ir,
{
    type Value = V;
    type Error = E;
    type Ext = ConcreteExt;
    type StageInfo = S;

    fn read_ref(&self, value: SSAValue) -> Result<&V, E> {
        self.read_ref_from_current_frame(value)
    }

    fn write(&mut self, result: ResultValue, value: V) -> Result<(), E> {
        self.write_to_current_frame(result, value)
    }

    fn write_ssa(&mut self, ssa: SSAValue, value: V) -> Result<(), E> {
        self.write_ssa_to_current_frame(ssa, value)
    }

    fn pipeline(&self) -> &'ir Pipeline<S> {
        self.pipeline
    }

    fn active_stage(&self) -> CompileStage {
        self.active_stage_from_frames()
    }
}
