use kirin_ir::StageMeta;
use smallvec::SmallVec;

use super::StackInterpreter;
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
    pub fn advance(&mut self, control: &Continuation<V, ConcreteExt>) -> Result<(), E> {
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
    pub fn run(&mut self) -> Result<Continuation<V, ConcreteExt>, E> {
        self.drive_loop(
            false,
            true,
            |interp| interp.step(),
            |interp, control| interp.advance(control),
        )
    }

    /// Stage-dynamic entrypoint.
    pub fn run_until_break(&mut self) -> Result<Continuation<V, ConcreteExt>, E> {
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
            if stop_on_breakpoint
                && let Some(cursor) = self.current_cursor()?
                && self.breakpoints.contains(&cursor)
            {
                return Ok(Continuation::Ext(ConcreteExt::Break));
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

    /// Drive execution handling nested Call/Return pairs.
    ///
    /// Runs until a `Return` or `Yield` continuation triggers exit (determined
    /// by `should_exit`). Nested `Call`s are tracked and their return values
    /// written back automatically with arity checking.
    ///
    /// `should_exit(interp, is_yield)` is called after `advance` for each
    /// `Return`/`Yield`. Return `true` to exit the loop with the values.
    pub(crate) fn run_nested_calls<F>(&mut self, mut should_exit: F) -> Result<SmallVec<[V; 1]>, E>
    where
        F: FnMut(&Self, bool) -> bool,
    {
        let mut pending_results: Vec<SmallVec<[kirin_ir::ResultValue; 1]>> = Vec::new();
        loop {
            let control = self.run()?;
            match &control {
                Continuation::Call { results, .. } => {
                    pending_results.push(results.clone());
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

            let values = match &control {
                Continuation::Return(vs) | Continuation::Yield(vs) => Some(vs.clone()),
                _ => None,
            };

            let is_yield = matches!(&control, Continuation::Yield(_));
            self.advance(&control)?;

            if let Some(values) = values {
                if should_exit(self, is_yield) {
                    return Ok(values);
                }
                let results = pending_results.pop().ok_or(InterpreterError::NoFrame)?;
                self.write_many(&results, &values)?;
            }
        }
    }
}
