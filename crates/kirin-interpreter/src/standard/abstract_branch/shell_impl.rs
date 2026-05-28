use kirin_ir::Dialect;

use super::AbstractBranchFrame;
use crate::{
    AbstractBlockTransfer, AbstractInterpreterWithStore, AbstractValue, ConcreteInterpreter, Env,
    FixpointProfile, Frame, FrameEffect, InterpreterError, InterpreterProfile, ProjectOrSelf,
    StandardCompletion, StandardFixpointInterpreter, StandardFrame,
};

mod sealed {
    pub trait Sealed {}
}

trait AbstractBranchShell<V>: sealed::Sealed + Env<V> {}

impl<'ir, P, Store> sealed::Sealed for AbstractInterpreterWithStore<'ir, P, Store> where
    P: InterpreterProfile
{
}

impl<'ir, P, Store> AbstractBranchShell<P::Value> for AbstractInterpreterWithStore<'ir, P, Store>
where
    P: InterpreterProfile,
    Store: Env<P::Value>,
    P::Error: From<Store::Error>,
{
}

impl<'ir, P, Store, Deps> sealed::Sealed for StandardFixpointInterpreter<'ir, P, Store, Deps> where
    P: FixpointProfile
{
}

impl<'ir, P, Store, Deps> AbstractBranchShell<P::Value>
    for StandardFixpointInterpreter<'ir, P, Store, Deps>
where
    P: FixpointProfile,
    Store: Env<P::Value>,
    P::Error: From<Store::Error>,
{
}

impl<I, L, F, C, E, V> Frame<I, F, C, E> for AbstractBranchFrame<L, V>
where
    I: AbstractBranchShell<V, Error = E>,
    L: Dialect,
    F: TryFrom<StandardFrame<L, V, AbstractBlockTransfer<V>>>,
    C: TryFrom<StandardCompletion<V>> + ProjectOrSelf<StandardCompletion<V>>,
    E: From<InterpreterError>
        + From<<F as TryFrom<StandardFrame<L, V, AbstractBlockTransfer<V>>>>::Error>
        + From<<C as TryFrom<StandardCompletion<V>>>::Error>,
    V: AbstractValue,
{
    fn step(self, _interp: &mut I) -> Result<FrameEffect<F, C>, E> {
        self.step_abstract()
    }

    fn resume_done(self, _interp: &mut I) -> Result<FrameEffect<F, C>, E> {
        self.resume_done_abstract()
    }

    fn resume(self, completion: C, interp: &mut I) -> Result<FrameEffect<F, C>, E> {
        self.resume_abstract(completion, interp)
    }
}

impl<'ir, P, L, F, C, E, V> Frame<ConcreteInterpreter<'ir, P>, F, C, E>
    for AbstractBranchFrame<L, V>
where
    P: InterpreterProfile,
    E: From<InterpreterError>,
{
    fn step(self, _interp: &mut ConcreteInterpreter<'ir, P>) -> Result<FrameEffect<F, C>, E> {
        Err(E::from(InterpreterError::Custom(
            "concrete interpreter cannot run abstract branch frame",
        )))
    }

    fn resume_done(
        self,
        _interp: &mut ConcreteInterpreter<'ir, P>,
    ) -> Result<FrameEffect<F, C>, E> {
        Err(E::from(InterpreterError::Custom(
            "concrete interpreter cannot run abstract branch frame",
        )))
    }

    fn resume(
        self,
        completion: C,
        _interp: &mut ConcreteInterpreter<'ir, P>,
    ) -> Result<FrameEffect<F, C>, E> {
        Ok(FrameEffect::Complete(completion))
    }
}
