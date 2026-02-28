use kirin_ir::{Dialect, HasStageInfo, StageMeta, SupportsStageDispatch};

use super::{DynFrameDispatch, FrameDispatchAction, PushCallFrameDynAction, StackInterpreter};
use crate::{ConcreteContinuation, ConcreteExt, Continuation, Interpretable, InterpreterError};

// -- Execution engine -------------------------------------------------------

impl<'ir, V, S, E, G> StackInterpreter<'ir, V, S, E, G>
where
    V: Clone + 'ir,
    E: From<InterpreterError> + 'ir,
    S: StageMeta + 'ir,
    G: 'ir,
{
    /// Execute the current statement's dialect semantics.
    /// Returns the raw [`ConcreteContinuation`] without advancing the cursor.
    pub fn step_in_stage<L>(&mut self) -> Result<ConcreteContinuation<V>, E>
    where
        S: HasStageInfo<L>,
        L: Dialect + Interpretable<'ir, Self, L> + 'ir,
    {
        let stage_id = self.current_frame_stage()?;
        self.step_with_stage_id::<L>(stage_id)
    }

    /// Stage-dynamic entrypoint.
    pub fn step(&mut self) -> Result<ConcreteContinuation<V>, E> {
        let dispatch = self.current_frame_dispatch()?;
        (dispatch.step)(self)
    }

    /// Apply cursor mutations for a continuation with strict typed-stage
    /// checking on the current frame stage.
    pub fn advance_in_stage<L>(&mut self, control: &ConcreteContinuation<V>) -> Result<(), E>
    where
        S: HasStageInfo<L>,
        S: SupportsStageDispatch<
                FrameDispatchAction<'ir, V, S, E, G>,
                DynFrameDispatch<'ir, V, S, E, G>,
                E,
            >,
        for<'a> S: SupportsStageDispatch<PushCallFrameDynAction<'a, 'ir, V, S, E, G>, (), E>,
        L: Dialect + Interpretable<'ir, Self, L> + 'ir,
    {
        let stage_id = self.current_frame_stage()?;
        self.advance_frame_with_stage_id::<L>(stage_id, control)?;
        if let Continuation::Call {
            callee,
            stage: callee_stage,
            args,
            ..
        } = control
        {
            self.push_call_frame_with_args(*callee, *callee_stage, args)?;
        }
        Ok(())
    }

    /// Stage-dynamic entrypoint.
    pub fn advance(&mut self, control: &ConcreteContinuation<V>) -> Result<(), E>
    where
        S: SupportsStageDispatch<
                FrameDispatchAction<'ir, V, S, E, G>,
                DynFrameDispatch<'ir, V, S, E, G>,
                E,
            >,
        for<'a> S: SupportsStageDispatch<PushCallFrameDynAction<'a, 'ir, V, S, E, G>, (), E>,
    {
        let dispatch = self.current_frame_dispatch()?;
        (dispatch.advance)(self, control)?;
        if let Continuation::Call {
            callee,
            stage: callee_stage,
            args,
            ..
        } = control
        {
            self.push_call_frame_with_args(*callee, *callee_stage, args)?;
        }
        Ok(())
    }

    /// Run statements until Return, Halt, or Call.
    /// Ignores breakpoints and Break from dialect intrinsics.
    pub fn run_in_stage<L>(&mut self) -> Result<ConcreteContinuation<V>, E>
    where
        S: HasStageInfo<L>,
        S: SupportsStageDispatch<
                FrameDispatchAction<'ir, V, S, E, G>,
                DynFrameDispatch<'ir, V, S, E, G>,
                E,
            >,
        for<'a> S: SupportsStageDispatch<PushCallFrameDynAction<'a, 'ir, V, S, E, G>, (), E>,
        L: Dialect + Interpretable<'ir, Self, L> + 'ir,
    {
        self.drive_loop(
            false,
            true,
            |interp| interp.step_in_stage::<L>(),
            |interp, control| interp.advance_in_stage::<L>(control),
        )
    }

    /// Stage-dynamic entrypoint.
    pub fn run(&mut self) -> Result<ConcreteContinuation<V>, E>
    where
        S: SupportsStageDispatch<
                FrameDispatchAction<'ir, V, S, E, G>,
                DynFrameDispatch<'ir, V, S, E, G>,
                E,
            >,
        for<'a> S: SupportsStageDispatch<PushCallFrameDynAction<'a, 'ir, V, S, E, G>, (), E>,
    {
        self.drive_loop(
            false,
            true,
            |interp| interp.step(),
            |interp, control| interp.advance(control),
        )
    }

    /// Run statements until a breakpoint, Return, Halt, or Call.
    pub fn run_until_break_in_stage<L>(&mut self) -> Result<ConcreteContinuation<V>, E>
    where
        S: HasStageInfo<L>,
        S: SupportsStageDispatch<
                FrameDispatchAction<'ir, V, S, E, G>,
                DynFrameDispatch<'ir, V, S, E, G>,
                E,
            >,
        for<'a> S: SupportsStageDispatch<PushCallFrameDynAction<'a, 'ir, V, S, E, G>, (), E>,
        L: Dialect + Interpretable<'ir, Self, L> + 'ir,
    {
        self.drive_loop(
            true,
            false,
            |interp| interp.step_in_stage::<L>(),
            |interp, control| interp.advance_in_stage::<L>(control),
        )
    }

    /// Stage-dynamic entrypoint.
    pub fn run_until_break(&mut self) -> Result<ConcreteContinuation<V>, E>
    where
        S: SupportsStageDispatch<
                FrameDispatchAction<'ir, V, S, E, G>,
                DynFrameDispatch<'ir, V, S, E, G>,
                E,
            >,
        for<'a> S: SupportsStageDispatch<PushCallFrameDynAction<'a, 'ir, V, S, E, G>, (), E>,
    {
        self.drive_loop(
            true,
            false,
            |interp| interp.step(),
            |interp, control| interp.advance(control),
        )
    }

    fn drive_loop<Step, Advance>(
        &mut self,
        stop_on_breakpoint: bool,
        swallow_break: bool,
        mut step_fn: Step,
        mut advance_fn: Advance,
    ) -> Result<ConcreteContinuation<V>, E>
    where
        Step: FnMut(&mut Self) -> Result<ConcreteContinuation<V>, E>,
        Advance: FnMut(&mut Self, &ConcreteContinuation<V>) -> Result<(), E>,
    {
        loop {
            if stop_on_breakpoint {
                if let Some(cursor) = self.current_cursor()? {
                    if self.breakpoints.contains(&cursor) {
                        return Ok(Continuation::Ext(ConcreteExt::Break));
                    }
                }
            }

            let control = step_fn(self)?;
            match &control {
                Continuation::Continue | Continuation::Jump(..) => advance_fn(self, &control)?,
                Continuation::Ext(ConcreteExt::Break) if swallow_break => {
                    advance_fn(self, &Continuation::Continue)?
                }
                _ => return Ok(control),
            }
        }
    }
}
