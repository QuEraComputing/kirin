use kirin_ir::{ResultValue, StageMeta, SupportsStageDispatch};

use super::{DynFrameDispatch, FrameDispatchAction, PushCallFrameDynAction, StackInterpreter};
use crate::{ConcreteExt, Continuation, InterpreterError, ValueStore};

// -- Execution engine -------------------------------------------------------

impl<'ir, V, S, E, G> StackInterpreter<'ir, V, S, E, G>
where
    V: Clone + 'ir,
    E: From<InterpreterError> + 'ir,
    S: StageMeta + 'ir,
    G: 'ir,
{
    /// Stage-dynamic entrypoint.
    pub fn step(&mut self) -> Result<Continuation<V, ConcreteExt>, E> {
        let dispatch = self.frames.current()?.extra().dispatch;
        (dispatch.step)(self)
    }

    /// Stage-dynamic entrypoint.
    pub fn advance(&mut self, control: &Continuation<V, ConcreteExt>) -> Result<(), E>
    where
        S: SupportsStageDispatch<
                FrameDispatchAction<'ir, V, S, E, G>,
                DynFrameDispatch<'ir, V, S, E, G>,
                E,
            >,
        for<'a> S: SupportsStageDispatch<PushCallFrameDynAction<'a, 'ir, V, S, E, G>, (), E>,
    {
        let dispatch = self.frames.current()?.extra().dispatch;
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
    pub fn run(&mut self) -> Result<Continuation<V, ConcreteExt>, E>
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
    pub fn run_until_break(&mut self) -> Result<Continuation<V, ConcreteExt>, E>
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
    ) -> Result<Continuation<V, ConcreteExt>, E>
    where
        Step: FnMut(&mut Self) -> Result<Continuation<V, ConcreteExt>, E>,
        Advance: FnMut(&mut Self, &Continuation<V, ConcreteExt>) -> Result<(), E>,
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

    /// Like [`advance`](Self::advance) but uses the pre-built dispatch table
    /// for call-frame pushes, avoiding `SupportsStageDispatch` bounds.
    fn advance_cached(&mut self, control: &Continuation<V, ConcreteExt>) -> Result<(), E> {
        let dispatch = self.frames.current()?.extra().dispatch;
        (dispatch.advance)(self, control)?;
        if let Continuation::Call {
            callee,
            stage: callee_stage,
            args,
            ..
        } = control
        {
            self.push_call_frame_with_args_cached(*callee, *callee_stage, args)?;
        }
        Ok(())
    }

    /// Like [`run`](Self::run) but uses cached dispatch (no
    /// `SupportsStageDispatch` bounds).
    fn run_cached(&mut self) -> Result<Continuation<V, ConcreteExt>, E> {
        self.drive_loop(
            false,
            true,
            |interp| interp.step(),
            |interp, control| interp.advance_cached(control),
        )
    }

    /// Like [`run_nested_calls`](Self::run_nested_calls) but uses cached
    /// dispatch (no `SupportsStageDispatch` bounds).
    pub(crate) fn run_nested_calls_cached<F>(&mut self, mut should_exit: F) -> Result<V, E>
    where
        F: FnMut(&Self, bool) -> bool,
    {
        let mut pending_results: Vec<ResultValue> = Vec::new();
        loop {
            let control = self.run_cached()?;
            match &control {
                Continuation::Call { result, .. } => {
                    pending_results.push(*result);
                }
                Continuation::Return(_) | Continuation::Yield(_) => {}
                Continuation::Ext(ConcreteExt::Halt) => {
                    return Err(InterpreterError::UnexpectedControl(
                        "halt during nested call".to_owned(),
                    )
                    .into());
                }
                _ => {
                    return Err(InterpreterError::UnexpectedControl(
                        "unexpected continuation in nested call loop".to_owned(),
                    )
                    .into());
                }
            }

            let v = match &control {
                Continuation::Return(v) | Continuation::Yield(v) => Some(v.clone()),
                _ => None,
            };

            let is_yield = matches!(&control, Continuation::Yield(_));
            self.advance_cached(&control)?;

            if let Some(v) = v {
                if should_exit(self, is_yield) {
                    return Ok(v);
                }
                let result = pending_results.pop().ok_or(InterpreterError::NoFrame)?;
                ValueStore::write(self, result, v)?;
            }
        }
    }

    /// Drive execution handling nested Call/Return pairs.
    ///
    /// Runs until a `Return` or `Yield` continuation triggers exit (determined
    /// by `should_exit`). Nested `Call`s are tracked and their return values
    /// written back automatically.
    ///
    /// `should_exit(interp, is_yield)` is called after `advance` for each
    /// `Return`/`Yield`. Return `true` to exit the loop with the value.
    pub(crate) fn run_nested_calls<F>(&mut self, mut should_exit: F) -> Result<V, E>
    where
        S: SupportsStageDispatch<
                FrameDispatchAction<'ir, V, S, E, G>,
                DynFrameDispatch<'ir, V, S, E, G>,
                E,
            >,
        for<'a> S: SupportsStageDispatch<PushCallFrameDynAction<'a, 'ir, V, S, E, G>, (), E>,
        F: FnMut(&Self, bool) -> bool,
    {
        let mut pending_results: Vec<ResultValue> = Vec::new();
        loop {
            let control = self.run()?;
            match &control {
                Continuation::Call { result, .. } => {
                    pending_results.push(*result);
                }
                Continuation::Return(_) | Continuation::Yield(_) => {}
                Continuation::Ext(ConcreteExt::Halt) => {
                    return Err(InterpreterError::UnexpectedControl(
                        "halt during nested call".to_owned(),
                    )
                    .into());
                }
                _ => {
                    return Err(InterpreterError::UnexpectedControl(
                        "unexpected continuation in nested call loop".to_owned(),
                    )
                    .into());
                }
            }

            let v = match &control {
                Continuation::Return(v) | Continuation::Yield(v) => Some(v.clone()),
                _ => None,
            };

            let is_yield = matches!(&control, Continuation::Yield(_));
            self.advance(&control)?;

            if let Some(v) = v {
                if should_exit(self, is_yield) {
                    return Ok(v);
                }
                let result = pending_results.pop().ok_or(InterpreterError::NoFrame)?;
                ValueStore::write(self, result, v)?;
            }
        }
    }
}
