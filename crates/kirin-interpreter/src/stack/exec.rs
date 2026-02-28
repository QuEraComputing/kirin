use kirin_ir::{StageMeta, SupportsStageDispatch};

use super::{DynFrameDispatch, FrameDispatchAction, PushCallFrameDynAction, StackInterpreter};
use crate::{ConcreteContinuation, ConcreteExt, Continuation, InterpreterError};

// -- Execution engine -------------------------------------------------------

impl<'ir, V, S, E, G> StackInterpreter<'ir, V, S, E, G>
where
    V: Clone + 'ir,
    E: From<InterpreterError> + 'ir,
    S: StageMeta + 'ir,
    G: 'ir,
{
    /// Stage-dynamic entrypoint.
    pub fn step(&mut self) -> Result<ConcreteContinuation<V>, E> {
        let dispatch = self.call_stack.current()?.extra().dispatch;
        (dispatch.step)(self)
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
        let dispatch = self.call_stack.current()?.extra().dispatch;
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

    pub(super) fn drive_loop<Step, Advance>(
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
