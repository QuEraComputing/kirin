use std::marker::PhantomData;

use kirin_ir::{
    Dialect, Function, LiftFrom, Product, SSAValue, SpecializedFunction, StagedFunction, TryLift,
    TryLiftFrom,
};

use crate::{
    Env, EnvIndex, Frame, FrameEffect, HasLocation, InterpreterError, Location, ProjectOrSelf,
    StandardCompletion,
};

use super::{FunctionFrame, SpecializedFunctionFrame, StagedFunctionFrame};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Callee {
    Function(Function),
    StagedFunction(StagedFunction),
    SpecializedFunction(SpecializedFunction),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CallFrame<L, V> {
    pub location: Location,
    pub callee: Callee,
    pub args: Product<SSAValue>,
    pub caller_env: EnvIndex,
    pub results: Product<SSAValue>,
    _marker: PhantomData<fn() -> (L, V)>,
}

impl<L, V> CallFrame<L, V> {
    pub fn new(
        location: Location,
        callee: Callee,
        args: Product<SSAValue>,
        caller_env: EnvIndex,
        results: Product<SSAValue>,
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
    I: Env<V, Error = E>,
    L: Dialect,
    F: TryLiftFrom<CallFrame<L, V>>
        + TryLiftFrom<FunctionFrame<L, V>>
        + TryLiftFrom<StagedFunctionFrame<L, V>>
        + TryLiftFrom<SpecializedFunctionFrame<L, V>>,
    C: TryLiftFrom<StandardCompletion<V>> + ProjectOrSelf<StandardCompletion<V>>,
    E: LiftFrom<InterpreterError>
        + From<<F as TryLiftFrom<CallFrame<L, V>>>::Error>
        + From<<F as TryLiftFrom<FunctionFrame<L, V>>>::Error>
        + From<<F as TryLiftFrom<StagedFunctionFrame<L, V>>>::Error>
        + From<<F as TryLiftFrom<SpecializedFunctionFrame<L, V>>>::Error>
        + From<<C as TryLiftFrom<StandardCompletion<V>>>::Error>,
{
    fn step(self, interp: &mut I) -> Result<FrameEffect<F, C>, E> {
        let args = self
            .args
            .iter()
            .map(|arg| interp.read(self.caller_env, *arg))
            .collect::<Result<Product<_>, _>>()?;
        let stage = self.location.stage;
        let child = match self.callee {
            Callee::Function(function) => {
                FunctionFrame::<L, V>::new(stage, function, args).try_lift()?
            }
            Callee::StagedFunction(function) => {
                StagedFunctionFrame::<L, V>::new(stage, function, args).try_lift()?
            }
            Callee::SpecializedFunction(function) => {
                SpecializedFunctionFrame::<L, V>::new(stage, function, args).try_lift()?
            }
        };
        Ok(FrameEffect::Push {
            parent: self.try_lift()?,
            child,
        })
    }

    fn resume_done(self, _interp: &mut I) -> Result<FrameEffect<F, C>, E> {
        Ok(FrameEffect::Done)
    }

    fn resume(self, completion: C, interp: &mut I) -> Result<FrameEffect<F, C>, E> {
        match completion.project_or_self() {
            Ok(StandardCompletion::FunctionReturned(value)) => {
                interp.write_product(self.caller_env, self.results.as_slice(), value)?;
                Ok(FrameEffect::Done)
            }
            Ok(completion) => Err(E::lift_from(InterpreterError::UnexpectedCompletion {
                location: self.location,
                completion: match completion {
                    StandardCompletion::BlockDone => "block completion returned to call",
                    StandardCompletion::RegionDone => "region completion returned to call",
                    StandardCompletion::GraphDone => "graph completion returned to call",
                    StandardCompletion::FunctionReturned(_) => unreachable!(),
                },
            })),
            Err(completion) => Ok(FrameEffect::Complete(completion)),
        }
    }
}
