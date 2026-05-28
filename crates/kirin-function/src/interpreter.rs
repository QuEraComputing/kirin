use kirin::prelude::{
    CompileTimeValue, Dialect, HasArguments, HasRegionBody, HasResults, HasStageInfo, Product,
    SSAValue, Symbol,
};
use kirin_interpreter::{
    AbstractBlockTransfer, AbstractInterpreterWithStore, BlockTransfer, CallFrame, Callee,
    ConcreteBlockTransfer, ConcreteInterpreter, Env, EnvIndex, FixpointProfile, FunctionEntry,
    FunctionEntryTarget, Interpretable, InterpreterError, InterpreterProfile, Location,
    RegionFrame, StageAccess, StandardCompletion, StandardFixpointInterpreter, StandardFrame,
    StatementEffect,
};

use crate::{
    Bind, CallFunction, CallLike, CallNamed, CallSpecialized, CallStaged, Function, Lambda, Return,
};

pub trait CallTargetResolution<L: Dialect> {
    type Error;

    fn resolve_call_target(
        &self,
        location: Location,
        target: Symbol,
    ) -> Result<ResolvedCallTarget, Self::Error>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ResolvedCallTarget {
    pub stage: kirin::prelude::CompileStage,
    pub target: FunctionEntryTarget,
}

pub trait FunctionRegionDispatch<L: Dialect, F, E, V> {
    fn dispatch_function_region(
        &mut self,
        location: Location,
        region: kirin::prelude::Region,
        env: EnvIndex,
        args: Product<V>,
    ) -> Result<F, E>;
}

impl<'ir, P, L, F> FunctionRegionDispatch<L, F, P::Error, P::Value> for ConcreteInterpreter<'ir, P>
where
    P: InterpreterProfile,
    P::Stage: HasStageInfo<L>,
    L: Dialect,
    F: TryFrom<StandardFrame<L, P::Value, ConcreteBlockTransfer<P::Value>>>,
    P::Error:
        From<<F as TryFrom<StandardFrame<L, P::Value, ConcreteBlockTransfer<P::Value>>>>::Error>,
    P::Value: Clone,
{
    fn dispatch_function_region(
        &mut self,
        location: Location,
        region: kirin::prelude::Region,
        env: EnvIndex,
        args: Product<P::Value>,
    ) -> Result<F, P::Error> {
        StandardFrame::Region(
            RegionFrame::<L, P::Value, ConcreteBlockTransfer<P::Value>>::new(
                location.stage,
                region,
                env,
                args,
            ),
        )
        .try_into()
        .map_err(P::Error::from)
    }
}

impl<'ir, P, L, F, Store> FunctionRegionDispatch<L, F, P::Error, P::Value>
    for AbstractInterpreterWithStore<'ir, P, Store>
where
    P: InterpreterProfile,
    P::Stage: HasStageInfo<L>,
    L: Dialect,
    F: TryFrom<StandardFrame<L, P::Value, AbstractBlockTransfer<P::Value>>>,
    P::Error:
        From<<F as TryFrom<StandardFrame<L, P::Value, AbstractBlockTransfer<P::Value>>>>::Error>,
    P::Value: Clone,
{
    fn dispatch_function_region(
        &mut self,
        location: Location,
        region: kirin::prelude::Region,
        env: EnvIndex,
        args: Product<P::Value>,
    ) -> Result<F, P::Error> {
        StandardFrame::Region(
            RegionFrame::<L, P::Value, AbstractBlockTransfer<P::Value>>::new(
                location.stage,
                region,
                env,
                args,
            ),
        )
        .try_into()
        .map_err(P::Error::from)
    }
}

impl<'ir, P, L, F, Store, Deps> FunctionRegionDispatch<L, F, P::Error, P::Value>
    for StandardFixpointInterpreter<'ir, P, Store, Deps>
where
    P: FixpointProfile,
    P::Stage: HasStageInfo<L>,
    L: Dialect,
    F: TryFrom<StandardFrame<L, P::Value, AbstractBlockTransfer<P::Value>>>,
    P::Error:
        From<<F as TryFrom<StandardFrame<L, P::Value, AbstractBlockTransfer<P::Value>>>>::Error>,
    P::Value: Clone,
{
    fn dispatch_function_region(
        &mut self,
        location: Location,
        region: kirin::prelude::Region,
        env: EnvIndex,
        args: Product<P::Value>,
    ) -> Result<F, P::Error> {
        StandardFrame::Region(
            RegionFrame::<L, P::Value, AbstractBlockTransfer<P::Value>>::new(
                location.stage,
                region,
                env,
                args,
            ),
        )
        .try_into()
        .map_err(P::Error::from)
    }
}

impl<'ir, P, L> CallTargetResolution<L> for ConcreteInterpreter<'ir, P>
where
    P: InterpreterProfile,
    P::Stage: HasStageInfo<L>,
    L: Dialect,
    P::Error: From<InterpreterError>,
{
    type Error = P::Error;

    fn resolve_call_target(
        &self,
        location: Location,
        target: Symbol,
    ) -> Result<ResolvedCallTarget, Self::Error> {
        let stage = StageAccess::<L>::stage_info(self, location.stage)?;
        let function = self
            .pipeline()
            .resolve_function(stage, target)
            .ok_or(InterpreterError::MissingCallTarget { location, target })
            .map_err(P::Error::from)?;
        Ok(ResolvedCallTarget {
            stage: location.stage,
            target: FunctionEntryTarget::Function(function),
        })
    }
}

impl<'ir, P, L, Store> CallTargetResolution<L> for AbstractInterpreterWithStore<'ir, P, Store>
where
    P: InterpreterProfile,
    P::Stage: HasStageInfo<L>,
    L: Dialect,
    P::Error: From<InterpreterError>,
{
    type Error = P::Error;

    fn resolve_call_target(
        &self,
        location: Location,
        target: Symbol,
    ) -> Result<ResolvedCallTarget, Self::Error> {
        let stage = StageAccess::<L>::stage_info(self, location.stage)?;
        let function = self
            .pipeline()
            .resolve_function(stage, target)
            .ok_or(InterpreterError::MissingCallTarget { location, target })
            .map_err(P::Error::from)?;
        Ok(ResolvedCallTarget {
            stage: location.stage,
            target: FunctionEntryTarget::Function(function),
        })
    }
}

impl<L, I, F, E, V, T> FunctionEntry<L, I, F, E, V> for Function<T>
where
    L: Dialect,
    I: FunctionRegionDispatch<L, F, E, V>,
    T: CompileTimeValue,
    V: Clone,
{
    fn enter_function_body(
        &self,
        location: Location,
        env: EnvIndex,
        interp: &mut I,
        args: Product<V>,
    ) -> Result<F, E> {
        interp.dispatch_function_region(location, *self.region(), env, args)
    }
}

impl<L, I, F, E, V, T> FunctionEntry<L, I, F, E, V> for Lambda<T>
where
    L: Dialect,
    I: FunctionRegionDispatch<L, F, E, V>,
    T: CompileTimeValue,
    V: Clone,
{
    fn enter_function_body(
        &self,
        location: Location,
        env: EnvIndex,
        interp: &mut I,
        args: Product<V>,
    ) -> Result<F, E> {
        interp.dispatch_function_region(location, *self.region(), env, args)
    }
}

impl<L, I, F, C, E, T, X> Interpretable<L, I, F, C, E, X> for Function<T>
where
    L: Dialect,
    I: FunctionRegionDispatch<L, F, E, X::Value>,
    T: CompileTimeValue,
    X: BlockTransfer,
    X::Value: Clone,
{
    fn interpret(
        &self,
        location: Location,
        env: EnvIndex,
        interp: &mut I,
    ) -> Result<StatementEffect<F, C, X>, E> {
        interp
            .dispatch_function_region(location, *self.region(), env, Product::new())
            .map(StatementEffect::Push)
    }
}

impl<L, I, F, C, E, T, X> Interpretable<L, I, F, C, E, X> for Lambda<T>
where
    L: Dialect,
    I: FunctionRegionDispatch<L, F, E, X::Value>,
    T: CompileTimeValue,
    X: BlockTransfer,
    X::Value: Clone,
{
    fn interpret(
        &self,
        location: Location,
        env: EnvIndex,
        interp: &mut I,
    ) -> Result<StatementEffect<F, C, X>, E> {
        interp
            .dispatch_function_region(location, *self.region(), env, Product::new())
            .map(StatementEffect::Push)
    }
}

impl<L, I, F, C, E, T, X> Interpretable<L, I, F, C, E, X> for Bind<T>
where
    L: Dialect,
    E: From<InterpreterError>,
    T: CompileTimeValue,
{
    fn interpret(
        &self,
        _location: Location,
        _env: EnvIndex,
        _interp: &mut I,
    ) -> Result<StatementEffect<F, C, X>, E> {
        let _ = self;
        Err(E::from(InterpreterError::Custom(
            "bind is not yet supported in the new interpreter",
        )))
    }
}

impl<L, I, F, C, E, T, X> Interpretable<L, I, F, C, E, X> for CallNamed<T>
where
    L: Dialect,
    I: CallTargetResolution<L, Error = E>,
    F: TryFrom<CallFrame<L, X::Value>>,
    E: From<<F as TryFrom<CallFrame<L, X::Value>>>::Error>,
    T: CompileTimeValue,
    X: BlockTransfer,
{
    fn interpret(
        &self,
        location: Location,
        env: EnvIndex,
        interp: &mut I,
    ) -> Result<StatementEffect<F, C, X>, E> {
        let target_location =
            Location::new(self.stage().unwrap_or(location.stage), location.position);
        let target = interp.resolve_call_target(target_location, self.target())?;
        let args = self.arguments().copied().collect();
        let results = self.results().copied().map(SSAValue::from).collect();
        let callee = match target.target {
            FunctionEntryTarget::Function(function) => Callee::Function(function),
            FunctionEntryTarget::StagedFunction(function) => Callee::StagedFunction(function),
            FunctionEntryTarget::SpecializedFunction(function) => {
                Callee::SpecializedFunction(function)
            }
        };
        CallFrame::<L, X::Value>::new_in_stage(location, target.stage, callee, args, env, results)
            .try_into()
            .map(StatementEffect::Push)
            .map_err(E::from)
    }
}

impl<L, I, F, C, E, T, X> Interpretable<L, I, F, C, E, X> for CallFunction<T>
where
    L: Dialect,
    F: TryFrom<CallFrame<L, X::Value>>,
    E: From<<F as TryFrom<CallFrame<L, X::Value>>>::Error>,
    T: CompileTimeValue,
    X: BlockTransfer,
{
    fn interpret(
        &self,
        location: Location,
        env: EnvIndex,
        _interp: &mut I,
    ) -> Result<StatementEffect<F, C, X>, E> {
        direct_call_effect::<L, F, C, E, T, X, _>(
            location,
            env,
            self,
            Callee::Function(self.target()),
        )
    }
}

impl<L, I, F, C, E, T, X> Interpretable<L, I, F, C, E, X> for CallStaged<T>
where
    L: Dialect,
    F: TryFrom<CallFrame<L, X::Value>>,
    E: From<<F as TryFrom<CallFrame<L, X::Value>>>::Error>,
    T: CompileTimeValue,
    X: BlockTransfer,
{
    fn interpret(
        &self,
        location: Location,
        env: EnvIndex,
        _interp: &mut I,
    ) -> Result<StatementEffect<F, C, X>, E> {
        direct_call_effect::<L, F, C, E, T, X, _>(
            location,
            env,
            self,
            Callee::StagedFunction(self.target()),
        )
    }
}

impl<L, I, F, C, E, T, X> Interpretable<L, I, F, C, E, X> for CallSpecialized<T>
where
    L: Dialect,
    F: TryFrom<CallFrame<L, X::Value>>,
    E: From<<F as TryFrom<CallFrame<L, X::Value>>>::Error>,
    T: CompileTimeValue,
    X: BlockTransfer,
{
    fn interpret(
        &self,
        location: Location,
        env: EnvIndex,
        _interp: &mut I,
    ) -> Result<StatementEffect<F, C, X>, E> {
        direct_call_effect::<L, F, C, E, T, X, _>(
            location,
            env,
            self,
            Callee::SpecializedFunction(self.target()),
        )
    }
}

fn direct_call_effect<L, F, C, E, T, X, Target>(
    location: Location,
    env: EnvIndex,
    call: &impl CallLike<T, Target = Target>,
    callee: Callee,
) -> Result<StatementEffect<F, C, X>, E>
where
    L: Dialect,
    F: TryFrom<CallFrame<L, X::Value>>,
    E: From<<F as TryFrom<CallFrame<L, X::Value>>>::Error>,
    T: CompileTimeValue,
    X: BlockTransfer,
    Target: Copy,
{
    let stage = call.stage().unwrap_or(location.stage);
    let args = call.arguments().copied().collect();
    let results = call.results().copied().map(SSAValue::from).collect();
    CallFrame::<L, X::Value>::new_in_stage(location, stage, callee, args, env, results)
        .try_into()
        .map(StatementEffect::Push)
        .map_err(E::from)
}

impl<L, I, F, C, E, T, X> Interpretable<L, I, F, C, E, X> for Return<T>
where
    L: Dialect,
    I: Env<X::Value, Error = E>,
    C: TryFrom<StandardCompletion<X::Value>>,
    E: From<<C as TryFrom<StandardCompletion<X::Value>>>::Error>,
    T: CompileTimeValue,
    X: BlockTransfer,
{
    fn interpret(
        &self,
        _location: Location,
        env: EnvIndex,
        interp: &mut I,
    ) -> Result<StatementEffect<F, C, X>, E> {
        let values = interp.read_many(env, self.values.as_slice())?;
        Ok(StatementEffect::Complete(C::try_from(
            StandardCompletion::FunctionReturned(values),
        )?))
    }
}
