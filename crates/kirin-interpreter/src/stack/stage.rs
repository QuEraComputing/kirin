use kirin_ir::{
    Dialect, HasStageInfo, SpecializedFunction, StageInfo, StageMeta, SupportsStageDispatch,
};

use super::{DynFrameDispatch, FrameDispatchAction, PushCallFrameDynAction, StackInterpreter};
use crate::{
    ConcreteContinuation, Continuation, EvalCall, InStage, Interpretable, Interpreter,
    InterpreterError, WithStage,
};

impl<'a, 'ir, V, S, E, G, L> InStage<'a, StackInterpreter<'ir, V, S, E, G>, L>
where
    V: Clone + 'ir,
    E: From<InterpreterError> + 'ir,
    S: StageMeta + HasStageInfo<L> + 'ir,
    G: 'ir,
    L: Dialect + Interpretable<'ir, StackInterpreter<'ir, V, S, E, G>, L> + 'ir,
{
    fn resolve_current_frame_stage_info(&self) -> Result<&'ir StageInfo<L>, E> {
        let stage_id = self.interp.frames.current()?.stage();
        self.interp.resolve_stage_info::<L>(stage_id)
    }

    /// Execute the current statement's dialect semantics.
    /// Returns the raw [`ConcreteContinuation`] without advancing the cursor.
    pub fn step(self) -> Result<ConcreteContinuation<V>, E> {
        let stage = self.resolve_current_frame_stage_info()?;
        self.interp.with_stage(stage).step()
    }

    /// Apply cursor mutations for a continuation with strict typed-stage
    /// checking on the current frame stage.
    pub fn advance(self, control: &ConcreteContinuation<V>) -> Result<(), E>
    where
        S: SupportsStageDispatch<
                FrameDispatchAction<'ir, V, S, E, G>,
                DynFrameDispatch<'ir, V, S, E, G>,
                E,
            >,
        for<'b> S: SupportsStageDispatch<PushCallFrameDynAction<'b, 'ir, V, S, E, G>, (), E>,
    {
        let stage = self.resolve_current_frame_stage_info()?;
        self.interp.with_stage(stage).advance(control)
    }

    /// Call a specialized function and return its result value using strict
    /// typed-stage checking.
    pub fn call(self, callee: SpecializedFunction, args: &[V]) -> Result<V, E>
    where
        L: EvalCall<'ir, StackInterpreter<'ir, V, S, E, G>, L, Result = V>,
    {
        let stage = self.resolve_active_stage_info()?;
        self.interp.with_stage(stage).call(callee, args)
    }

    /// Run statements until Return, Halt, or Call.
    /// Ignores breakpoints and Break from dialect intrinsics.
    pub fn run(self) -> Result<ConcreteContinuation<V>, E>
    where
        S: SupportsStageDispatch<
                FrameDispatchAction<'ir, V, S, E, G>,
                DynFrameDispatch<'ir, V, S, E, G>,
                E,
            >,
        for<'b> S: SupportsStageDispatch<PushCallFrameDynAction<'b, 'ir, V, S, E, G>, (), E>,
    {
        self.interp.drive_loop(
            false,
            true,
            |interp: &mut StackInterpreter<'ir, V, S, E, G>| interp.in_stage::<L>().step(),
            |interp: &mut StackInterpreter<'ir, V, S, E, G>, control| {
                interp.in_stage::<L>().advance(control)
            },
        )
    }

    /// Run statements until a breakpoint, Return, Halt, or Call.
    pub fn run_until_break(self) -> Result<ConcreteContinuation<V>, E>
    where
        S: SupportsStageDispatch<
                FrameDispatchAction<'ir, V, S, E, G>,
                DynFrameDispatch<'ir, V, S, E, G>,
                E,
            >,
        for<'b> S: SupportsStageDispatch<PushCallFrameDynAction<'b, 'ir, V, S, E, G>, (), E>,
    {
        self.interp.drive_loop(
            true,
            false,
            |interp: &mut StackInterpreter<'ir, V, S, E, G>| interp.in_stage::<L>().step(),
            |interp: &mut StackInterpreter<'ir, V, S, E, G>, control| {
                interp.in_stage::<L>().advance(control)
            },
        )
    }
}

impl<'a, 'ir, V, S, E, G, L> WithStage<'a, 'ir, StackInterpreter<'ir, V, S, E, G>, L>
where
    V: Clone + 'ir,
    E: From<InterpreterError> + 'ir,
    S: StageMeta + 'ir,
    G: 'ir,
    L: Dialect + Interpretable<'ir, StackInterpreter<'ir, V, S, E, G>, L> + 'ir,
{
    /// Execute the current statement's dialect semantics for this stage.
    pub fn step(self) -> Result<ConcreteContinuation<V>, E> {
        self.interp.step_with_stage::<L>(self.stage)
    }

    /// Apply cursor mutations for a continuation with this explicit stage.
    pub fn advance(self, control: &ConcreteContinuation<V>) -> Result<(), E>
    where
        S: SupportsStageDispatch<
                FrameDispatchAction<'ir, V, S, E, G>,
                DynFrameDispatch<'ir, V, S, E, G>,
                E,
            >,
        for<'b> S: SupportsStageDispatch<PushCallFrameDynAction<'b, 'ir, V, S, E, G>, (), E>,
    {
        self.interp
            .advance_frame_with_stage::<L>(self.stage, control)?;
        if let Continuation::Call {
            callee,
            stage: callee_stage,
            args,
            ..
        } = control
        {
            self.interp
                .push_call_frame_with_args(*callee, *callee_stage, args)?;
        }
        Ok(())
    }

    /// Call a specialized function and return its result value for this stage.
    pub fn call(self, callee: SpecializedFunction, args: &[V]) -> Result<V, E>
    where
        L: EvalCall<'ir, StackInterpreter<'ir, V, S, E, G>, L, Result = V>,
    {
        self.interp.call_with_stage::<L>(callee, self.stage, args)
    }

    pub(crate) fn push_call_frame(self, callee: SpecializedFunction, args: &[V]) -> Result<(), E>
    where
        S: SupportsStageDispatch<
                FrameDispatchAction<'ir, V, S, E, G>,
                DynFrameDispatch<'ir, V, S, E, G>,
                E,
            >,
    {
        self.interp
            .push_call_frame_with_stage::<L>(callee, self.stage, args)
    }
}
