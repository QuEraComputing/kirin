use std::hash::Hash;

use kirin_ir::{Block, CompileStage, Dialect, TryLiftFrom};

use crate::{
    AbstractInterpreter, AbstractValue, ConcreteInterpreter, EnvIndex, FrameEffect,
    InterpreterError, SimpleFixpointInterpreter, StandardCompletion, Summary,
};

pub trait BlockBranchDispatch<L: Dialect, F, C, E, V> {
    fn dispatch_branch(
        &mut self,
        stage: CompileStage,
        env: EnvIndex,
        true_target: Block,
        true_arguments: Vec<V>,
        false_target: Block,
        false_arguments: Vec<V>,
    ) -> Result<FrameEffect<F, C>, E>;
}

impl<'ir, S, L, F, C, E, V> BlockBranchDispatch<L, F, C, E, V>
    for ConcreteInterpreter<'ir, S, F, C, E, V>
where
    L: Dialect,
    E: From<InterpreterError>,
{
    fn dispatch_branch(
        &mut self,
        _stage: CompileStage,
        _env: EnvIndex,
        _true_target: Block,
        _true_arguments: Vec<V>,
        _false_target: Block,
        _false_arguments: Vec<V>,
    ) -> Result<FrameEffect<F, C>, E> {
        Err(InterpreterError::Custom("concrete interpreter cannot branch abstractly").into())
    }
}

impl<'ir, S, L, F, C, E, V> BlockBranchDispatch<L, F, C, E, V>
    for AbstractInterpreter<'ir, S, F, C, E, V>
where
    L: Dialect,
    C: TryLiftFrom<StandardCompletion<V>>,
    E: From<InterpreterError> + From<<C as TryLiftFrom<StandardCompletion<V>>>::Error>,
    V: AbstractValue,
{
    fn dispatch_branch(
        &mut self,
        _stage: CompileStage,
        _env: EnvIndex,
        _true_target: Block,
        _true_arguments: Vec<V>,
        _false_target: Block,
        _false_arguments: Vec<V>,
    ) -> Result<FrameEffect<F, C>, E> {
        // The local abstract shell has no dependency graph yet, so an unknown CFG
        // branch conservatively summarizes the enclosing function result.
        Ok(FrameEffect::Complete(C::try_lift_from(
            StandardCompletion::FunctionReturned(V::top()),
        )?))
    }
}

impl<'ir, Stage, K, L, F, C, E, V, S, Store> BlockBranchDispatch<L, F, C, E, V>
    for SimpleFixpointInterpreter<'ir, Stage, K, F, C, E, S, Store>
where
    K: Clone + Eq + Hash,
    L: Dialect,
    S: Summary,
    C: TryLiftFrom<StandardCompletion<V>>,
    E: From<InterpreterError> + From<<C as TryLiftFrom<StandardCompletion<V>>>::Error>,
    V: AbstractValue,
{
    fn dispatch_branch(
        &mut self,
        _stage: CompileStage,
        _env: EnvIndex,
        _true_target: Block,
        _true_arguments: Vec<V>,
        _false_target: Block,
        _false_arguments: Vec<V>,
    ) -> Result<FrameEffect<F, C>, E> {
        Ok(FrameEffect::Complete(C::try_lift_from(
            StandardCompletion::FunctionReturned(V::top()),
        )?))
    }
}
