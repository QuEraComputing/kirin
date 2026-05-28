use kirin_ir::{CompileStage, Function, Product, SpecializedFunction, StagedFunction};

use crate::{
    AbstractInterpreterWithStore, ConcreteInterpreter, Env, Frame, FrameDispatch, InterpreterError,
    InterpreterProfile,
};

use super::{FunctionFrame, SpecializedFunctionFrame, StagedFunctionFrame};

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
        F: TryFrom<FunctionFrame<L, V>>
            + TryFrom<StagedFunctionFrame<L, V>>
            + TryFrom<SpecializedFunctionFrame<L, V>>,
        E: From<<F as TryFrom<FunctionFrame<L, V>>>::Error>
            + From<<F as TryFrom<StagedFunctionFrame<L, V>>>::Error>
            + From<<F as TryFrom<SpecializedFunctionFrame<L, V>>>::Error>,
    {
        match self.target {
            FunctionEntryTarget::Function(function) => {
                FunctionFrame::<L, V>::new(self.stage, function, self.args)
                    .try_into()
                    .map_err(E::from)
            }
            FunctionEntryTarget::StagedFunction(function) => {
                StagedFunctionFrame::<L, V>::new(self.stage, function, self.args)
                    .try_into()
                    .map_err(E::from)
            }
            FunctionEntryTarget::SpecializedFunction(function) => {
                SpecializedFunctionFrame::<L, V>::new(self.stage, function, self.args)
                    .try_into()
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

impl<'a, 'ir, P> FunctionInvokeTargetBuilder<'a, ConcreteInterpreter<'ir, P>>
where
    P: InterpreterProfile,
    ConcreteInterpreter<'ir, P>: FrameDispatch<P::Frame, P::Value, P::Error>,
    P::Frame: Frame<ConcreteInterpreter<'ir, P>, P::Frame, P::Completion, P::Error>,
    P::Error: From<InterpreterError>,
{
    pub fn args<A>(self, args: A) -> Result<P::Completion, P::Error>
    where
        A: IntoIterator<Item = P::Value>,
    {
        let invocation =
            FunctionInvocation::new(self.stage, self.target, args.into_iter().collect());
        let frame = self.interp.dispatch_function_invocation(invocation)?;
        self.interp.push_frame(frame);
        self.interp.run()
    }
}

impl<'a, 'ir, P, Store> FunctionInvokeTargetBuilder<'a, AbstractInterpreterWithStore<'ir, P, Store>>
where
    P: InterpreterProfile,
{
    pub fn args<A>(self, args: A) -> Result<P::Completion, P::Error>
    where
        Store: Env<P::Value>,
        AbstractInterpreterWithStore<'ir, P, Store>: FrameDispatch<P::Frame, P::Value, P::Error>,
        P::Frame:
            Frame<AbstractInterpreterWithStore<'ir, P, Store>, P::Frame, P::Completion, P::Error>,
        P::Error: From<InterpreterError>,
        A: IntoIterator<Item = P::Value>,
    {
        let invocation =
            FunctionInvocation::new(self.stage, self.target, args.into_iter().collect());
        let frame = self.interp.dispatch_function_invocation(invocation)?;
        self.interp.push_frame(frame);
        self.interp.run()
    }
}
