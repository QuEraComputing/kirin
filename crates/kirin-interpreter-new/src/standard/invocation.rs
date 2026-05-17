use kirin_ir::{
    CompileStage, Function, LiftFrom, Product, SpecializedFunction, StagedFunction, TryLift,
    TryLiftFrom,
};

use crate::{AbstractInterpreterWithStore, ConcreteInterpreter, Env, Frame, InterpreterError};

use super::{FunctionFrame, SpecializedFunctionFrame, StagedFunctionFrame, StandardFrame};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FunctionEntryTarget {
    Function(Function),
    StagedFunction(StagedFunction),
    SpecializedFunction(SpecializedFunction),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FunctionInvocation<V> {
    stage: CompileStage,
    target: FunctionEntryTarget,
    args: Product<V>,
}

pub trait FunctionInvocationFrame<V>: Sized {
    type Language;
    type Error;

    fn from_function_invocation(invocation: FunctionInvocation<V>) -> Result<Self, Self::Error>;
}

impl<L, V, T> FunctionInvocationFrame<V> for StandardFrame<L, V, T> {
    type Language = L;
    type Error = core::convert::Infallible;

    fn from_function_invocation(invocation: FunctionInvocation<V>) -> Result<Self, Self::Error> {
        invocation.into_root_frame::<L, Self, Self::Error>()
    }
}

impl<V> FunctionInvocation<V> {
    pub fn new(stage: CompileStage, target: FunctionEntryTarget, args: Product<V>) -> Self {
        Self {
            stage,
            target,
            args,
        }
    }

    pub fn function(stage: CompileStage, function: Function, args: Product<V>) -> Self {
        Self::new(stage, FunctionEntryTarget::Function(function), args)
    }

    pub fn staged_function(
        stage: CompileStage,
        function: StagedFunction,
        args: Product<V>,
    ) -> Self {
        Self::new(stage, FunctionEntryTarget::StagedFunction(function), args)
    }

    pub fn specialized_function(
        stage: CompileStage,
        function: SpecializedFunction,
        args: Product<V>,
    ) -> Self {
        Self::new(
            stage,
            FunctionEntryTarget::SpecializedFunction(function),
            args,
        )
    }

    pub fn stage(&self) -> CompileStage {
        self.stage
    }

    pub fn target(&self) -> FunctionEntryTarget {
        self.target
    }

    pub fn args(&self) -> &Product<V> {
        &self.args
    }

    pub fn into_root_frame<L, F, E>(self) -> Result<F, E>
    where
        F: TryLiftFrom<FunctionFrame<L, V>>
            + TryLiftFrom<StagedFunctionFrame<L, V>>
            + TryLiftFrom<SpecializedFunctionFrame<L, V>>,
        E: From<<F as TryLiftFrom<FunctionFrame<L, V>>>::Error>
            + From<<F as TryLiftFrom<StagedFunctionFrame<L, V>>>::Error>
            + From<<F as TryLiftFrom<SpecializedFunctionFrame<L, V>>>::Error>,
    {
        match self.target {
            FunctionEntryTarget::Function(function) => {
                FunctionFrame::<L, V>::new(self.stage, function, self.args)
                    .try_lift()
                    .map_err(E::from)
            }
            FunctionEntryTarget::StagedFunction(function) => {
                StagedFunctionFrame::<L, V>::new(self.stage, function, self.args)
                    .try_lift()
                    .map_err(E::from)
            }
            FunctionEntryTarget::SpecializedFunction(function) => {
                SpecializedFunctionFrame::<L, V>::new(self.stage, function, self.args)
                    .try_lift()
                    .map_err(E::from)
            }
        }
    }
}

pub struct FunctionInvokeBuilder<'a, I> {
    interp: &'a mut I,
    stage: CompileStage,
}

impl<'a, I> FunctionInvokeBuilder<'a, I> {
    pub fn new(interp: &'a mut I, stage: CompileStage) -> Self {
        Self { interp, stage }
    }

    pub fn function(self, function: Function) -> FunctionInvokeTargetBuilder<'a, I> {
        FunctionInvokeTargetBuilder::new(
            self.interp,
            self.stage,
            FunctionEntryTarget::Function(function),
        )
    }

    pub fn staged(self, function: StagedFunction) -> FunctionInvokeTargetBuilder<'a, I> {
        FunctionInvokeTargetBuilder::new(
            self.interp,
            self.stage,
            FunctionEntryTarget::StagedFunction(function),
        )
    }

    pub fn specialized(self, function: SpecializedFunction) -> FunctionInvokeTargetBuilder<'a, I> {
        FunctionInvokeTargetBuilder::new(
            self.interp,
            self.stage,
            FunctionEntryTarget::SpecializedFunction(function),
        )
    }
}

pub struct FunctionInvokeTargetBuilder<'a, I> {
    interp: &'a mut I,
    stage: CompileStage,
    target: FunctionEntryTarget,
}

impl<'a, I> FunctionInvokeTargetBuilder<'a, I> {
    fn new(interp: &'a mut I, stage: CompileStage, target: FunctionEntryTarget) -> Self {
        Self {
            interp,
            stage,
            target,
        }
    }
}

impl<'a, 'ir, S, F, C, E, V>
    FunctionInvokeTargetBuilder<'a, ConcreteInterpreter<'ir, S, F, C, E, V>>
where
    F: FunctionInvocationFrame<V> + Frame<ConcreteInterpreter<'ir, S, F, C, E, V>, F, C, E>,
    E: LiftFrom<InterpreterError> + From<<F as FunctionInvocationFrame<V>>::Error>,
{
    pub fn args<A>(self, args: A) -> Result<C, E>
    where
        A: IntoIterator<Item = V>,
    {
        let invocation =
            FunctionInvocation::new(self.stage, self.target, args.into_iter().collect());
        let frame = F::from_function_invocation(invocation).map_err(E::from)?;
        self.interp.push_frame(frame);
        self.interp.run()
    }
}

impl<'a, 'ir, S, F, C, E, Store>
    FunctionInvokeTargetBuilder<'a, AbstractInterpreterWithStore<'ir, S, F, C, E, Store>>
{
    pub fn args<V, A>(self, args: A) -> Result<C, E>
    where
        Store: Env<V>,
        F: FunctionInvocationFrame<V>
            + Frame<AbstractInterpreterWithStore<'ir, S, F, C, E, Store>, F, C, E>,
        E: LiftFrom<InterpreterError> + From<<F as FunctionInvocationFrame<V>>::Error>,
        A: IntoIterator<Item = V>,
    {
        let invocation =
            FunctionInvocation::new(self.stage, self.target, args.into_iter().collect());
        let frame = F::from_function_invocation(invocation).map_err(E::from)?;
        self.interp.push_frame(frame);
        self.interp.run()
    }
}
