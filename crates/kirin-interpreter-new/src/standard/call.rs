use std::marker::PhantomData;

use kirin_ir::{Dialect, Function, SSAValue, SpecializedFunction, StagedFunction, TryLiftFrom};

use crate::{
    Env, EnvIndex, Frame, FrameEffect, HasLocation, InterpreterError, Location, ProductValue,
    ProjectOrSelf, StandardCompletion,
};

use super::{FunctionFrame, SpecializedFunctionFrame, StagedFunctionFrame};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Callee {
    Function(Function),
    StagedFunction(StagedFunction),
    SpecializedFunction(SpecializedFunction),
}

pub trait CallResultBinding<V> {
    type Error;

    fn write_call_results(
        &mut self,
        location: Location,
        env: EnvIndex,
        results: &[SSAValue],
        value: V,
    ) -> Result<(), Self::Error>;
}

impl<I, V> CallResultBinding<V> for I
where
    I: Env<V>,
    V: ProductValue,
    <I as Env<V>>::Error: From<InterpreterError>,
{
    type Error = <I as Env<V>>::Error;

    fn write_call_results(
        &mut self,
        _location: Location,
        env: EnvIndex,
        results: &[SSAValue],
        value: V,
    ) -> Result<(), Self::Error> {
        self.write_product(env, results, value)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CallFrame<L, V> {
    pub location: Location,
    pub callee: Callee,
    pub args: Vec<SSAValue>,
    pub caller_env: EnvIndex,
    pub results: Vec<SSAValue>,
    _marker: PhantomData<fn() -> (L, V)>,
}

impl<L, V> CallFrame<L, V> {
    pub fn new(
        location: Location,
        callee: Callee,
        args: Vec<SSAValue>,
        caller_env: EnvIndex,
        results: Vec<SSAValue>,
    ) -> Self {
        Self {
            location,
            callee,
            args,
            caller_env,
            results,
            _marker: PhantomData,
        }
    }
}

impl<L, V> HasLocation for CallFrame<L, V> {
    fn location(&self) -> Location {
        self.location
    }
}

impl<I, L, F, C, E, V> Frame<I, F, C, E> for CallFrame<L, V>
where
    I: Env<V, Error = E> + CallResultBinding<V, Error = E>,
    L: Dialect,
    F: From<CallFrame<L, V>>
        + From<FunctionFrame<L, V>>
        + From<StagedFunctionFrame<L, V>>
        + From<SpecializedFunctionFrame<L, V>>,
    C: TryLiftFrom<StandardCompletion<V>> + ProjectOrSelf<StandardCompletion<V>>,
    E: From<InterpreterError> + From<<C as TryLiftFrom<StandardCompletion<V>>>::Error>,
{
    fn step(self, interp: &mut I) -> Result<FrameEffect<F, C>, E> {
        let args = self
            .args
            .iter()
            .map(|arg| interp.read(self.caller_env, *arg))
            .collect::<Result<Vec<_>, _>>()?;
        let stage = self.location.stage;
        let child = match self.callee {
            Callee::Function(function) => FunctionFrame::<L, V>::new(stage, function, args).into(),
            Callee::StagedFunction(function) => {
                StagedFunctionFrame::<L, V>::new(stage, function, args).into()
            }
            Callee::SpecializedFunction(function) => {
                SpecializedFunctionFrame::<L, V>::new(stage, function, args).into()
            }
        };
        Ok(FrameEffect::Push {
            parent: self.into(),
            child,
        })
    }

    fn resume_done(self, _interp: &mut I) -> Result<FrameEffect<F, C>, E> {
        Ok(FrameEffect::Done)
    }

    fn resume(self, completion: C, interp: &mut I) -> Result<FrameEffect<F, C>, E> {
        match completion.project_or_self() {
            Ok(StandardCompletion::FunctionReturned(value)) => {
                interp.write_call_results(
                    self.location,
                    self.caller_env,
                    self.results.as_slice(),
                    value,
                )?;
                Ok(FrameEffect::Done)
            }
            Ok(completion) => Err(InterpreterError::UnexpectedCompletion {
                location: self.location,
                completion: match completion {
                    StandardCompletion::BlockDone => "block completion returned to call",
                    StandardCompletion::RegionDone => "region completion returned to call",
                    StandardCompletion::GraphDone => "graph completion returned to call",
                    StandardCompletion::FunctionReturned(_) => unreachable!(),
                },
            }
            .into()),
            Err(completion) => Ok(FrameEffect::Complete(completion)),
        }
    }
}
