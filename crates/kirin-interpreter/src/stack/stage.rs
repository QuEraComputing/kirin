use kirin_ir::{Dialect, HasStageInfo, SpecializedFunction, StageMeta, SupportsStageDispatch};

use super::{DynFrameDispatch, FrameDispatchAction, PushCallFrameDynAction, StackInterpreter};
use crate::{
    CallSemantics, ConcreteExt, Continuation, Interpretable, InterpreterError, StageAccess, Staged,
};

impl<'a, 'ir, V, S, E, G, L> Staged<'a, 'ir, StackInterpreter<'ir, V, S, E, G>, L>
where
    V: Clone + 'ir,
    E: From<InterpreterError> + 'ir,
    S: StageMeta + 'ir,
    G: 'ir,
    L: Dialect + Interpretable<'ir, StackInterpreter<'ir, V, S, E, G>, L> + 'ir,
{
    /// Execute the current statement's dialect semantics.
    /// Returns the raw continuation without advancing the cursor.
    pub fn step(self) -> Result<Continuation<V, ConcreteExt>, E> {
        self.interp.step_with_stage::<L>(self.stage)
    }

    /// Apply cursor mutations for a continuation.
    pub fn advance(self, control: &Continuation<V, ConcreteExt>) -> Result<(), E>
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

    /// Call a specialized function and return its result value.
    pub fn call(self, callee: SpecializedFunction, args: &[V]) -> Result<V, E>
    where
        L: CallSemantics<'ir, StackInterpreter<'ir, V, S, E, G>, L, Result = V>,
    {
        self.interp.call_with_stage::<L>(callee, self.stage, args)
    }

    /// Run statements until Return, Halt, or Call.
    /// Ignores breakpoints and Break from dialect intrinsics.
    pub fn run(self) -> Result<Continuation<V, ConcreteExt>, E>
    where
        S: HasStageInfo<L>
            + SupportsStageDispatch<
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
    pub fn run_until_break(self) -> Result<Continuation<V, ConcreteExt>, E>
    where
        S: HasStageInfo<L>
            + SupportsStageDispatch<
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
