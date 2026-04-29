use std::hash::Hash;

use kirin_ir::{Dialect, TryLiftFrom};

use super::AbstractBranchFrame;
use crate::{
    AbstractInterpreter, AbstractValue, BlockFrame, ConcreteInterpreter, Env, Frame, FrameEffect,
    InterpreterError, ProjectOrSelf, SimpleFixpointInterpreter, StandardCompletion, Summary,
};

impl<'ir, S, L, F, C, E, V> Frame<AbstractInterpreter<'ir, S, F, C, E, V>, F, C, E>
    for AbstractBranchFrame<L, V>
where
    L: Dialect,
    F: From<AbstractBranchFrame<L, V>> + From<BlockFrame<L, V>>,
    C: TryLiftFrom<StandardCompletion<V>> + ProjectOrSelf<StandardCompletion<V>>,
    E: From<InterpreterError> + From<<C as TryLiftFrom<StandardCompletion<V>>>::Error>,
    V: AbstractValue,
{
    fn step(
        self,
        _interp: &mut AbstractInterpreter<'ir, S, F, C, E, V>,
    ) -> Result<FrameEffect<F, C>, E> {
        self.step_abstract()
    }

    fn resume_done(
        self,
        _interp: &mut AbstractInterpreter<'ir, S, F, C, E, V>,
    ) -> Result<FrameEffect<F, C>, E> {
        self.resume_done_abstract()
    }

    fn resume(
        self,
        completion: C,
        interp: &mut AbstractInterpreter<'ir, S, F, C, E, V>,
    ) -> Result<FrameEffect<F, C>, E> {
        self.resume_abstract(completion, interp)
    }
}

impl<'ir, Stage, K, L, F, C, E, V, Sum, Store>
    Frame<SimpleFixpointInterpreter<'ir, Stage, K, F, C, E, Sum, Store>, F, C, E>
    for AbstractBranchFrame<L, V>
where
    K: Clone + Eq + Hash,
    L: Dialect,
    F: From<AbstractBranchFrame<L, V>> + From<BlockFrame<L, V>>,
    C: TryLiftFrom<StandardCompletion<V>> + ProjectOrSelf<StandardCompletion<V>>,
    E: From<InterpreterError>
        + From<<C as TryLiftFrom<StandardCompletion<V>>>::Error>
        + From<<Store as Env<V>>::Error>,
    V: AbstractValue,
    Sum: Summary,
    Store: Env<V>,
{
    fn step(
        self,
        _interp: &mut SimpleFixpointInterpreter<'ir, Stage, K, F, C, E, Sum, Store>,
    ) -> Result<FrameEffect<F, C>, E> {
        self.step_abstract()
    }

    fn resume_done(
        self,
        _interp: &mut SimpleFixpointInterpreter<'ir, Stage, K, F, C, E, Sum, Store>,
    ) -> Result<FrameEffect<F, C>, E> {
        self.resume_done_abstract()
    }

    fn resume(
        self,
        completion: C,
        interp: &mut SimpleFixpointInterpreter<'ir, Stage, K, F, C, E, Sum, Store>,
    ) -> Result<FrameEffect<F, C>, E> {
        self.resume_abstract(completion, interp)
    }
}

impl<'ir, S, L, F, C, E, V, StoreValue>
    Frame<ConcreteInterpreter<'ir, S, F, C, E, StoreValue>, F, C, E> for AbstractBranchFrame<L, V>
where
    E: From<InterpreterError>,
{
    fn step(
        self,
        _interp: &mut ConcreteInterpreter<'ir, S, F, C, E, StoreValue>,
    ) -> Result<FrameEffect<F, C>, E> {
        Err(
            InterpreterError::Custom("concrete interpreter cannot run abstract branch frame")
                .into(),
        )
    }

    fn resume_done(
        self,
        _interp: &mut ConcreteInterpreter<'ir, S, F, C, E, StoreValue>,
    ) -> Result<FrameEffect<F, C>, E> {
        Err(
            InterpreterError::Custom("concrete interpreter cannot run abstract branch frame")
                .into(),
        )
    }

    fn resume(
        self,
        completion: C,
        _interp: &mut ConcreteInterpreter<'ir, S, F, C, E, StoreValue>,
    ) -> Result<FrameEffect<F, C>, E> {
        Ok(FrameEffect::Complete(completion))
    }
}
