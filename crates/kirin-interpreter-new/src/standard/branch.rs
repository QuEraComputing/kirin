use std::hash::Hash;

use kirin_ir::{Block, CompileStage, Dialect, TryLiftFrom};

use crate::{
    AbstractBranchFrame, AbstractInterpreterWithStore, AbstractValue, ConcreteInterpreter, Env,
    EnvIndex, ForkEnv, FrameEffect, InterpreterError, SimpleFixpointInterpreter,
    StandardCompletion, Summary,
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

impl<'ir, S, L, F, C, E, V, Store> BlockBranchDispatch<L, F, C, E, V>
    for AbstractInterpreterWithStore<'ir, S, F, C, E, Store>
where
    L: Dialect,
    F: From<AbstractBranchFrame<L, V>>,
    Store: ForkEnv<V>,
    C: TryLiftFrom<StandardCompletion<V>>,
    E: From<InterpreterError>
        + From<<C as TryLiftFrom<StandardCompletion<V>>>::Error>
        + From<Store::Error>,
    V: AbstractValue,
{
    fn dispatch_branch(
        &mut self,
        stage: CompileStage,
        env: EnvIndex,
        true_target: Block,
        true_arguments: Vec<V>,
        false_target: Block,
        false_arguments: Vec<V>,
    ) -> Result<FrameEffect<F, C>, E> {
        let true_env = self.fork_env(env)?;
        let false_env = self.fork_env(env)?;
        Ok(FrameEffect::Continue(
            AbstractBranchFrame::<L, V>::new(
                stage,
                true_env,
                true_target,
                true_arguments,
                false_env,
                false_target,
                false_arguments,
            )
            .into(),
        ))
    }
}

impl<'ir, Stage, K, L, F, C, E, V, S, Store> BlockBranchDispatch<L, F, C, E, V>
    for SimpleFixpointInterpreter<'ir, Stage, K, F, C, E, S, Store>
where
    K: Clone + Eq + Hash,
    L: Dialect,
    F: From<AbstractBranchFrame<L, V>>,
    S: Summary,
    Store: ForkEnv<V>,
    C: TryLiftFrom<StandardCompletion<V>>,
    E: From<InterpreterError>
        + From<<C as TryLiftFrom<StandardCompletion<V>>>::Error>
        + From<<Store as Env<V>>::Error>,
    V: AbstractValue,
{
    fn dispatch_branch(
        &mut self,
        stage: CompileStage,
        env: EnvIndex,
        true_target: Block,
        true_arguments: Vec<V>,
        false_target: Block,
        false_arguments: Vec<V>,
    ) -> Result<FrameEffect<F, C>, E> {
        let true_env = self.fork_env(env)?;
        let false_env = self.fork_env(env)?;
        Ok(FrameEffect::Continue(
            AbstractBranchFrame::<L, V>::new(
                stage,
                true_env,
                true_target,
                true_arguments,
                false_env,
                false_target,
                false_arguments,
            )
            .into(),
        ))
    }
}
