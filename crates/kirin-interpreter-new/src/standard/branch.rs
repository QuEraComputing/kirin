use std::hash::Hash;

use kirin_ir::{CompileStage, Dialect, LiftFrom, TryLift, TryLiftFrom};

use crate::{
    AbstractBlockTransfer, AbstractBranchFrame, AbstractInterpreterWithStore, AbstractValue,
    ConcreteBlockTransfer, ConcreteInterpreter, Env, EnvIndex, ForkEnv, FrameEffect,
    InterpreterError, StandardCompletion, StandardFixpointInterpreter, StandardFrame, Summary,
};

pub trait BlockTransferDispatch<L: Dialect, F, C, E, V, T> {
    fn dispatch_block_transfer(
        &mut self,
        stage: CompileStage,
        env: EnvIndex,
        transfer: T,
    ) -> Result<FrameEffect<F, C>, E>;
}

impl<'ir, S, L, F, C, E, V, RootF> BlockTransferDispatch<L, F, C, E, V, ConcreteBlockTransfer<V>>
    for ConcreteInterpreter<'ir, S, RootF, C, E, V>
where
    L: Dialect,
    F: TryLiftFrom<StandardFrame<L, V, ConcreteBlockTransfer<V>>>,
    E: LiftFrom<InterpreterError>
        + From<<F as TryLiftFrom<StandardFrame<L, V, ConcreteBlockTransfer<V>>>>::Error>,
{
    fn dispatch_block_transfer(
        &mut self,
        stage: CompileStage,
        env: EnvIndex,
        transfer: ConcreteBlockTransfer<V>,
    ) -> Result<FrameEffect<F, C>, E> {
        match transfer {
            ConcreteBlockTransfer::Jump { target, arguments } => {
                StandardFrame::Block(crate::BlockFrame::<L, V, ConcreteBlockTransfer<V>>::new(
                    stage, target, env, arguments,
                ))
                .try_lift()
                .map(FrameEffect::Continue)
                .map_err(E::from)
            }
        }
    }
}

impl<'ir, S, L, F, C, E, V, Store, RootF>
    BlockTransferDispatch<L, F, C, E, V, AbstractBlockTransfer<V>>
    for AbstractInterpreterWithStore<'ir, S, RootF, C, E, Store>
where
    L: Dialect,
    F: TryLiftFrom<StandardFrame<L, V, AbstractBlockTransfer<V>>>,
    Store: ForkEnv<V>,
    C: TryLiftFrom<StandardCompletion<V>>,
    E: LiftFrom<InterpreterError>
        + From<<F as TryLiftFrom<StandardFrame<L, V, AbstractBlockTransfer<V>>>>::Error>
        + From<<C as TryLiftFrom<StandardCompletion<V>>>::Error>
        + LiftFrom<Store::Error>,
    V: AbstractValue,
{
    fn dispatch_block_transfer(
        &mut self,
        stage: CompileStage,
        env: EnvIndex,
        transfer: AbstractBlockTransfer<V>,
    ) -> Result<FrameEffect<F, C>, E> {
        match transfer {
            AbstractBlockTransfer::Jump { target, arguments } => {
                StandardFrame::Block(crate::BlockFrame::<L, V, AbstractBlockTransfer<V>>::new(
                    stage, target, env, arguments,
                ))
                .try_lift()
                .map(FrameEffect::Continue)
                .map_err(E::from)
            }
            AbstractBlockTransfer::Branch {
                true_target,
                true_arguments,
                false_target,
                false_arguments,
            } => {
                let true_env = self.fork_env(env)?;
                let false_env = self.fork_env(env)?;
                StandardFrame::AbstractBranch(AbstractBranchFrame::<L, V>::new(
                    stage,
                    true_env,
                    true_target,
                    true_arguments,
                    false_env,
                    false_target,
                    false_arguments,
                ))
                .try_lift()
                .map(FrameEffect::Continue)
                .map_err(E::from)
            }
        }
    }
}

impl<'ir, Stage, K, L, F, C, E, V, S, Store, Deps, RootF>
    BlockTransferDispatch<L, F, C, E, V, AbstractBlockTransfer<V>>
    for StandardFixpointInterpreter<'ir, Stage, K, RootF, C, E, S, Store, Deps>
where
    K: Clone + Eq + Hash,
    L: Dialect,
    F: TryLiftFrom<StandardFrame<L, V, AbstractBlockTransfer<V>>>,
    S: Summary,
    Store: ForkEnv<V>,
    C: TryLiftFrom<StandardCompletion<V>>,
    E: LiftFrom<InterpreterError>
        + From<<F as TryLiftFrom<StandardFrame<L, V, AbstractBlockTransfer<V>>>>::Error>
        + From<<C as TryLiftFrom<StandardCompletion<V>>>::Error>
        + LiftFrom<<Store as Env<V>>::Error>,
    V: AbstractValue,
{
    fn dispatch_block_transfer(
        &mut self,
        stage: CompileStage,
        env: EnvIndex,
        transfer: AbstractBlockTransfer<V>,
    ) -> Result<FrameEffect<F, C>, E> {
        match transfer {
            AbstractBlockTransfer::Jump { target, arguments } => {
                StandardFrame::Block(crate::BlockFrame::<L, V, AbstractBlockTransfer<V>>::new(
                    stage, target, env, arguments,
                ))
                .try_lift()
                .map(FrameEffect::Continue)
                .map_err(E::from)
            }
            AbstractBlockTransfer::Branch {
                true_target,
                true_arguments,
                false_target,
                false_arguments,
            } => {
                let true_env = self.fork_env(env)?;
                let false_env = self.fork_env(env)?;
                StandardFrame::AbstractBranch(AbstractBranchFrame::<L, V>::new(
                    stage,
                    true_env,
                    true_target,
                    true_arguments,
                    false_env,
                    false_target,
                    false_arguments,
                ))
                .try_lift()
                .map(FrameEffect::Continue)
                .map_err(E::from)
            }
        }
    }
}
