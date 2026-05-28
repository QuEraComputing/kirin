use kirin_ir::{CompileStage, Dialect};

use crate::{
    AbstractBlockTransfer, AbstractBranchFrame, AbstractInterpreterWithStore, AbstractValue,
    ConcreteBlockTransfer, ConcreteInterpreter, Env, EnvIndex, FixpointProfile, ForkEnv,
    FrameEffect, InterpreterError, InterpreterProfile, StandardCompletion,
    StandardFixpointInterpreter, StandardFrame,
};

pub trait BlockTransferDispatch<L: Dialect, F, C, E, V, T> {
    fn dispatch_block_transfer(
        &mut self,
        stage: CompileStage,
        env: EnvIndex,
        transfer: T,
    ) -> Result<FrameEffect<F, C>, E>;
}

impl<'ir, P, L, F, C, E>
    BlockTransferDispatch<L, F, C, E, P::Value, ConcreteBlockTransfer<P::Value>>
    for ConcreteInterpreter<'ir, P>
where
    P: InterpreterProfile,
    L: Dialect,
    F: TryFrom<StandardFrame<L, P::Value, ConcreteBlockTransfer<P::Value>>>,
    E: From<InterpreterError>
        + From<<F as TryFrom<StandardFrame<L, P::Value, ConcreteBlockTransfer<P::Value>>>>::Error>,
{
    fn dispatch_block_transfer(
        &mut self,
        stage: CompileStage,
        env: EnvIndex,
        transfer: ConcreteBlockTransfer<P::Value>,
    ) -> Result<FrameEffect<F, C>, E> {
        match transfer {
            ConcreteBlockTransfer::Jump { target, arguments } => {
                StandardFrame::Block(crate::BlockFrame::<
                    L,
                    P::Value,
                    ConcreteBlockTransfer<P::Value>,
                >::new(stage, target, env, arguments))
                .try_into()
                .map(FrameEffect::Continue)
                .map_err(E::from)
            }
        }
    }
}

impl<'ir, P, L, F, C, Store>
    BlockTransferDispatch<L, F, C, P::Error, P::Value, AbstractBlockTransfer<P::Value>>
    for AbstractInterpreterWithStore<'ir, P, Store>
where
    P: InterpreterProfile,
    L: Dialect,
    F: TryFrom<StandardFrame<L, P::Value, AbstractBlockTransfer<P::Value>>>,
    Store: ForkEnv<P::Value>,
    C: TryFrom<StandardCompletion<P::Value>>,
    P::Error: From<InterpreterError>
        + From<<F as TryFrom<StandardFrame<L, P::Value, AbstractBlockTransfer<P::Value>>>>::Error>
        + From<<C as TryFrom<StandardCompletion<P::Value>>>::Error>
        + From<Store::Error>,
    P::Value: AbstractValue,
{
    fn dispatch_block_transfer(
        &mut self,
        stage: CompileStage,
        env: EnvIndex,
        transfer: AbstractBlockTransfer<P::Value>,
    ) -> Result<FrameEffect<F, C>, P::Error> {
        match transfer {
            AbstractBlockTransfer::Jump { target, arguments } => {
                StandardFrame::Block(crate::BlockFrame::<
                    L,
                    P::Value,
                    AbstractBlockTransfer<P::Value>,
                >::new(stage, target, env, arguments))
                .try_into()
                .map(FrameEffect::Continue)
                .map_err(P::Error::from)
            }
            AbstractBlockTransfer::Branch {
                true_target,
                true_arguments,
                false_target,
                false_arguments,
            } => {
                let true_env = self.fork_env(env)?;
                let false_env = self.fork_env(env)?;
                StandardFrame::AbstractBranch(AbstractBranchFrame::<L, P::Value>::new(
                    stage,
                    true_env,
                    true_target,
                    true_arguments,
                    false_env,
                    false_target,
                    false_arguments,
                ))
                .try_into()
                .map(FrameEffect::Continue)
                .map_err(P::Error::from)
            }
        }
    }
}

impl<'ir, P, L, F, C, Store, Deps>
    BlockTransferDispatch<L, F, C, P::Error, P::Value, AbstractBlockTransfer<P::Value>>
    for StandardFixpointInterpreter<'ir, P, Store, Deps>
where
    P: FixpointProfile,
    L: Dialect,
    F: TryFrom<StandardFrame<L, P::Value, AbstractBlockTransfer<P::Value>>>,
    Store: ForkEnv<P::Value>,
    C: TryFrom<StandardCompletion<P::Value>>,
    P::Error: From<InterpreterError>
        + From<<F as TryFrom<StandardFrame<L, P::Value, AbstractBlockTransfer<P::Value>>>>::Error>
        + From<<C as TryFrom<StandardCompletion<P::Value>>>::Error>
        + From<<Store as Env<P::Value>>::Error>,
    P::Value: AbstractValue,
{
    fn dispatch_block_transfer(
        &mut self,
        stage: CompileStage,
        env: EnvIndex,
        transfer: AbstractBlockTransfer<P::Value>,
    ) -> Result<FrameEffect<F, C>, P::Error> {
        match transfer {
            AbstractBlockTransfer::Jump { target, arguments } => {
                StandardFrame::Block(crate::BlockFrame::<
                    L,
                    P::Value,
                    AbstractBlockTransfer<P::Value>,
                >::new(stage, target, env, arguments))
                .try_into()
                .map(FrameEffect::Continue)
                .map_err(P::Error::from)
            }
            AbstractBlockTransfer::Branch {
                true_target,
                true_arguments,
                false_target,
                false_arguments,
            } => {
                let true_env = self.fork_env(env)?;
                let false_env = self.fork_env(env)?;
                StandardFrame::AbstractBranch(AbstractBranchFrame::<L, P::Value>::new(
                    stage,
                    true_env,
                    true_target,
                    true_arguments,
                    false_env,
                    false_target,
                    false_arguments,
                ))
                .try_into()
                .map(FrameEffect::Continue)
                .map_err(P::Error::from)
            }
        }
    }
}
