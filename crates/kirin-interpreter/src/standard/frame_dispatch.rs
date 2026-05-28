//! Interpreter-side frame dispatch.
//!
//! [`FrameDispatch`] is the single trait the framework expects an interpreter
//! to implement. It merges the old `FunctionInvocationDispatch` (function-
//! entry frames) and `StageBlockDispatch` (fixpoint block frames) into one
//! capability: "given a `CompileStage`, build the right root frame".
//!
//! Users almost never implement this trait themselves: the framework provides
//! blanket impls for `ConcreteInterpreter`, `AbstractInterpreterWithStore`,
//! and `StandardFixpointInterpreter` whenever the frame implements
//! [`StageFrame`](crate::StageFrame).

use kirin_ir::{Block, CompileStage, Product};

use crate::{
    AbstractInterpreterWithStore, ConcreteInterpreter, Env, EnvIndex, FixpointProfile,
    FunctionInvocation, InterpreterError, InterpreterProfile, StandardFixpointInterpreter,
};

use super::StageFrame;

/// Build root frames for the interpreter from runtime stage information.
pub trait FrameDispatch<F, V, E> {
    fn dispatch_function_invocation(&mut self, invocation: FunctionInvocation<V>) -> Result<F, E>;

    fn dispatch_block(
        &mut self,
        stage: CompileStage,
        block: Block,
        env: EnvIndex,
        args: Product<V>,
    ) -> Result<F, E>;
}

impl<'ir, P, F> FrameDispatch<F, P::Value, P::Error> for ConcreteInterpreter<'ir, P>
where
    P: InterpreterProfile,
    F: StageFrame<P::Stage, P::Value>,
    P::Error: From<F::Error> + From<InterpreterError>,
{
    fn dispatch_function_invocation(
        &mut self,
        invocation: FunctionInvocation<P::Value>,
    ) -> Result<F, P::Error> {
        let stage = invocation.stage();
        let stage_info = self
            .pipeline()
            .stage(stage)
            .ok_or(InterpreterError::MissingStage(stage))?;
        F::from_function_invocation(stage_info, invocation).map_err(P::Error::from)
    }

    fn dispatch_block(
        &mut self,
        stage: CompileStage,
        block: Block,
        env: EnvIndex,
        args: Product<P::Value>,
    ) -> Result<F, P::Error> {
        let stage_info = self
            .pipeline()
            .stage(stage)
            .ok_or(InterpreterError::MissingStage(stage))?;
        F::from_block(stage_info, stage, block, env, args).map_err(P::Error::from)
    }
}

impl<'ir, P, Store, F> FrameDispatch<F, P::Value, P::Error>
    for AbstractInterpreterWithStore<'ir, P, Store>
where
    P: InterpreterProfile,
    Store: Env<P::Value>,
    F: StageFrame<P::Stage, P::Value>,
    P::Error: From<F::Error> + From<InterpreterError>,
{
    fn dispatch_function_invocation(
        &mut self,
        invocation: FunctionInvocation<P::Value>,
    ) -> Result<F, P::Error> {
        let stage = invocation.stage();
        let stage_info = self
            .pipeline()
            .stage(stage)
            .ok_or(InterpreterError::MissingStage(stage))?;
        F::from_function_invocation(stage_info, invocation).map_err(P::Error::from)
    }

    fn dispatch_block(
        &mut self,
        stage: CompileStage,
        block: Block,
        env: EnvIndex,
        args: Product<P::Value>,
    ) -> Result<F, P::Error> {
        let stage_info = self
            .pipeline()
            .stage(stage)
            .ok_or(InterpreterError::MissingStage(stage))?;
        F::from_block(stage_info, stage, block, env, args).map_err(P::Error::from)
    }
}

impl<'ir, P, Store, Deps, F> FrameDispatch<F, P::Value, P::Error>
    for StandardFixpointInterpreter<'ir, P, Store, Deps>
where
    P: FixpointProfile,
    F: StageFrame<P::Stage, P::Value>,
    P::Error: From<F::Error> + From<InterpreterError>,
{
    fn dispatch_function_invocation(
        &mut self,
        invocation: FunctionInvocation<P::Value>,
    ) -> Result<F, P::Error> {
        let stage = invocation.stage();
        let stage_info = self
            .pipeline()
            .stage(stage)
            .ok_or(InterpreterError::MissingStage(stage))?;
        F::from_function_invocation(stage_info, invocation).map_err(P::Error::from)
    }

    fn dispatch_block(
        &mut self,
        stage: CompileStage,
        block: Block,
        env: EnvIndex,
        args: Product<P::Value>,
    ) -> Result<F, P::Error> {
        let stage_info = self
            .pipeline()
            .stage(stage)
            .ok_or(InterpreterError::MissingStage(stage))?;
        F::from_block(stage_info, stage, block, env, args).map_err(P::Error::from)
    }
}
