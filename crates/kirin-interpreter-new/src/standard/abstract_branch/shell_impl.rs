use std::hash::Hash;

use kirin_ir::{Dialect, LiftFrom, TryLiftFrom};

use super::AbstractBranchFrame;
use crate::{
    AbstractBlockTransfer, AbstractInterpreterWithStore, AbstractValue, ConcreteInterpreter, Env,
    Frame, FrameEffect, InterpreterError, ProjectOrSelf, StandardCompletion,
    StandardFixpointInterpreter, StandardFrame, Summary,
};

mod sealed {
    pub trait Sealed {}
}

trait AbstractBranchShell<V>: sealed::Sealed + Env<V> {}

impl<'ir, S, F, C, E, Store> sealed::Sealed
    for AbstractInterpreterWithStore<'ir, S, F, C, E, Store>
{
}

impl<'ir, S, F, C, E, V, Store> AbstractBranchShell<V>
    for AbstractInterpreterWithStore<'ir, S, F, C, E, Store>
where
    Store: Env<V>,
    E: LiftFrom<Store::Error>,
{
}

impl<'ir, Stage, K, F, C, E, Sum, Store, Deps> sealed::Sealed
    for StandardFixpointInterpreter<'ir, Stage, K, F, C, E, Sum, Store, Deps>
where
    Sum: Summary,
{
}

impl<'ir, Stage, K, F, C, E, V, Sum, Store, Deps> AbstractBranchShell<V>
    for StandardFixpointInterpreter<'ir, Stage, K, F, C, E, Sum, Store, Deps>
where
    K: Clone + Eq + Hash,
    Sum: Summary,
    Store: Env<V>,
    E: LiftFrom<Store::Error>,
{
}

impl<I, L, F, C, E, V> Frame<I, F, C, E> for AbstractBranchFrame<L, V>
where
    I: AbstractBranchShell<V, Error = E>,
    L: Dialect,
    F: TryLiftFrom<StandardFrame<L, V, AbstractBlockTransfer<V>>>,
    C: TryLiftFrom<StandardCompletion<V>> + ProjectOrSelf<StandardCompletion<V>>,
    E: LiftFrom<InterpreterError>
        + From<<F as TryLiftFrom<StandardFrame<L, V, AbstractBlockTransfer<V>>>>::Error>
        + From<<C as TryLiftFrom<StandardCompletion<V>>>::Error>,
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

impl<'ir, S, L, F, C, E, V, StoreValue, RootF>
    Frame<ConcreteInterpreter<'ir, S, RootF, C, E, StoreValue>, F, C, E>
    for AbstractBranchFrame<L, V>
where
    E: LiftFrom<InterpreterError>,
{
    fn step(
        self,
        _interp: &mut ConcreteInterpreter<'ir, S, RootF, C, E, StoreValue>,
    ) -> Result<FrameEffect<F, C>, E> {
        Err(E::lift_from(InterpreterError::Custom(
            "concrete interpreter cannot run abstract branch frame",
        )))
    }

    fn resume_done(
        self,
        _interp: &mut ConcreteInterpreter<'ir, S, RootF, C, E, StoreValue>,
    ) -> Result<FrameEffect<F, C>, E> {
        Err(E::lift_from(InterpreterError::Custom(
            "concrete interpreter cannot run abstract branch frame",
        )))
    }

    fn resume(
        self,
        completion: C,
        _interp: &mut ConcreteInterpreter<'ir, S, RootF, C, E, StoreValue>,
    ) -> Result<FrameEffect<F, C>, E> {
        Ok(FrameEffect::Complete(completion))
    }
}
