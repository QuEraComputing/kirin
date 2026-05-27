use std::marker::PhantomData;

use kirin_ir::{Dialect, Function, Product, SSAValue, SpecializedFunction, StagedFunction};

use crate::{
    Env, EnvIndex, Frame, FrameEffect, FunctionEntryTarget, FunctionInvocation,
    FunctionInvocationDispatch, HasLocation, InterpreterError, Location, ProjectOrSelf,
    StandardCompletion,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Callee {
    Function(Function),
    StagedFunction(StagedFunction),
    SpecializedFunction(SpecializedFunction),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CallFrame<L, V> {
    pub location: Location,
    pub callee_stage: kirin_ir::CompileStage,
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
        Self::new_in_stage(location, location.stage, callee, args, caller_env, results)
    }

    pub fn new_in_stage(
        location: Location,
        callee_stage: kirin_ir::CompileStage,
        callee: Callee,
        args: Product<SSAValue>,
        caller_env: EnvIndex,
        results: Product<SSAValue>,
    ) -> Self {
        Self {
            location,
            callee_stage,
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
    I: Env<V, Error = E> + FunctionInvocationDispatch<F, E, V>,
    L: Dialect,
    F: TryFrom<CallFrame<L, V>>,
    C: TryFrom<StandardCompletion<V>> + ProjectOrSelf<StandardCompletion<V>>,
    E: From<InterpreterError>
        + From<<F as TryFrom<CallFrame<L, V>>>::Error>
        + From<<C as TryFrom<StandardCompletion<V>>>::Error>,
{
    fn step(self, interp: &mut I) -> Result<FrameEffect<F, C>, E> {
        let args = self
            .args
            .iter()
            .map(|arg| interp.read(self.caller_env, *arg))
            .collect::<Result<Product<_>, _>>()?;
        let target = match self.callee {
            Callee::Function(function) => FunctionEntryTarget::Function(function),
            Callee::StagedFunction(function) => FunctionEntryTarget::StagedFunction(function),
            Callee::SpecializedFunction(function) => {
                FunctionEntryTarget::SpecializedFunction(function)
            }
        };
        let child = interp.dispatch_function_invocation(FunctionInvocation::new(
            self.callee_stage,
            target,
            args,
        ))?;
        Ok(FrameEffect::Push {
            parent: self.try_into()?,
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
            Ok(completion) => Err(E::from(InterpreterError::UnexpectedCompletion {
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
