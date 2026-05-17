use std::hash::Hash;

use kirin::ir::{LiftFrom, TryLift, TryLiftFrom};
use kirin::prelude::{
    CompileTimeValue, Dialect, Function as IrFunction, HasRegionBody, HasStageInfo, Product,
    SSAValue, Symbol,
};
use kirin_interpreter_new::{
    AbstractBlockTransfer, AbstractInterpreterWithStore, BlockTransfer, CallFrame, Callee,
    ConcreteBlockTransfer, ConcreteInterpreter, Env, EnvIndex, FunctionEntry, Interpretable,
    InterpreterError, Location, RegionFrame, StageAccess, StandardCompletion,
    StandardFixpointInterpreter, StatementEffect, Summary,
};

use crate::{Bind, Call, Function, Lambda, Lexical, Lifted, Return};

pub trait CallTargetResolution<L: Dialect> {
    type Error;

    fn resolve_call_target(
        &self,
        location: Location,
        target: Symbol,
    ) -> Result<IrFunction, Self::Error>;
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

impl<'ir, S, L, F, C, E, V> FunctionRegionDispatch<L, F, E, V>
    for ConcreteInterpreter<'ir, S, F, C, E, V>
where
    S: HasStageInfo<L>,
    L: Dialect,
    F: TryLiftFrom<RegionFrame<L, V, ConcreteBlockTransfer<V>>>,
    E: From<<F as TryLiftFrom<RegionFrame<L, V, ConcreteBlockTransfer<V>>>>::Error>,
    V: Clone,
{
    fn dispatch_function_region(
        &mut self,
        location: Location,
        region: kirin::prelude::Region,
        env: EnvIndex,
        args: Product<V>,
    ) -> Result<F, E> {
        RegionFrame::<L, V, ConcreteBlockTransfer<V>>::new(location.stage, region, env, args)
            .try_lift()
            .map_err(E::from)
    }
}

impl<'ir, S, L, F, C, E, V, Store> FunctionRegionDispatch<L, F, E, V>
    for AbstractInterpreterWithStore<'ir, S, F, C, E, Store>
where
    S: HasStageInfo<L>,
    L: Dialect,
    F: TryLiftFrom<RegionFrame<L, V, AbstractBlockTransfer<V>>>,
    E: From<<F as TryLiftFrom<RegionFrame<L, V, AbstractBlockTransfer<V>>>>::Error>,
    V: Clone,
{
    fn dispatch_function_region(
        &mut self,
        location: Location,
        region: kirin::prelude::Region,
        env: EnvIndex,
        args: Product<V>,
    ) -> Result<F, E> {
        RegionFrame::<L, V, AbstractBlockTransfer<V>>::new(location.stage, region, env, args)
            .try_lift()
            .map_err(E::from)
    }
}

impl<'ir, S, K, L, F, C, E, V, Sum, Store, Deps> FunctionRegionDispatch<L, F, E, V>
    for StandardFixpointInterpreter<'ir, S, K, F, C, E, Sum, Store, Deps>
where
    S: HasStageInfo<L>,
    K: Clone + Eq + Hash,
    L: Dialect,
    Sum: Summary,
    F: TryLiftFrom<RegionFrame<L, V, AbstractBlockTransfer<V>>>,
    E: From<<F as TryLiftFrom<RegionFrame<L, V, AbstractBlockTransfer<V>>>>::Error>,
    V: Clone,
{
    fn dispatch_function_region(
        &mut self,
        location: Location,
        region: kirin::prelude::Region,
        env: EnvIndex,
        args: Product<V>,
    ) -> Result<F, E> {
        RegionFrame::<L, V, AbstractBlockTransfer<V>>::new(location.stage, region, env, args)
            .try_lift()
            .map_err(E::from)
    }
}

impl<'ir, S, L, F, C, E, V> CallTargetResolution<L> for ConcreteInterpreter<'ir, S, F, C, E, V>
where
    S: HasStageInfo<L>,
    L: Dialect,
    E: LiftFrom<InterpreterError>,
{
    type Error = E;

    fn resolve_call_target(
        &self,
        location: Location,
        target: Symbol,
    ) -> Result<IrFunction, Self::Error> {
        let stage = StageAccess::<L>::stage_info(self, location.stage)?;
        self.pipeline()
            .resolve_function(stage, target)
            .ok_or(InterpreterError::MissingCallTarget { location, target })
            .map_err(E::lift_from)
    }
}

impl<'ir, S, L, F, C, E, Store> CallTargetResolution<L>
    for AbstractInterpreterWithStore<'ir, S, F, C, E, Store>
where
    S: HasStageInfo<L>,
    L: Dialect,
    E: LiftFrom<InterpreterError>,
{
    type Error = E;

    fn resolve_call_target(
        &self,
        location: Location,
        target: Symbol,
    ) -> Result<IrFunction, Self::Error> {
        let stage = StageAccess::<L>::stage_info(self, location.stage)?;
        self.pipeline()
            .resolve_function(stage, target)
            .ok_or(InterpreterError::MissingCallTarget { location, target })
            .map_err(E::lift_from)
    }
}

impl<'ir, S, K, L, F, C, E, Sum, Store, Deps> CallTargetResolution<L>
    for StandardFixpointInterpreter<'ir, S, K, F, C, E, Sum, Store, Deps>
where
    S: HasStageInfo<L>,
    K: Clone + Eq + Hash,
    L: Dialect,
    Sum: Summary,
    E: LiftFrom<InterpreterError>,
{
    type Error = E;

    fn resolve_call_target(
        &self,
        location: Location,
        target: Symbol,
    ) -> Result<IrFunction, Self::Error> {
        let stage = StageAccess::<L>::stage_info(self, location.stage)?;
        self.pipeline()
            .resolve_function(stage, target)
            .ok_or(InterpreterError::MissingCallTarget { location, target })
            .map_err(E::lift_from)
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
    E: LiftFrom<InterpreterError>,
    T: CompileTimeValue,
{
    fn interpret(
        &self,
        _location: Location,
        _env: EnvIndex,
        _interp: &mut I,
    ) -> Result<StatementEffect<F, C, X>, E> {
        let _ = self;
        Err(E::lift_from(InterpreterError::Custom(
            "bind is not yet supported in the new interpreter",
        )))
    }
}

impl<L, I, F, C, E, T, X> Interpretable<L, I, F, C, E, X> for Call<T>
where
    L: Dialect,
    I: CallTargetResolution<L, Error = E>,
    F: TryLiftFrom<CallFrame<L, X::Value>>,
    E: From<<F as TryLiftFrom<CallFrame<L, X::Value>>>::Error>,
    T: CompileTimeValue,
    X: BlockTransfer,
{
    fn interpret(
        &self,
        location: Location,
        env: EnvIndex,
        interp: &mut I,
    ) -> Result<StatementEffect<F, C, X>, E> {
        let function = interp.resolve_call_target(location, self.target())?;
        let args = self.args().iter().copied().collect();
        let results = self.results().iter().copied().map(SSAValue::from).collect();
        CallFrame::<L, X::Value>::new(location, Callee::Function(function), args, env, results)
            .try_lift()
            .map(StatementEffect::Push)
            .map_err(E::from)
    }
}

impl<L, I, F, C, E, T, X> Interpretable<L, I, F, C, E, X> for Return<T>
where
    L: Dialect,
    I: Env<X::Value, Error = E>,
    C: TryLiftFrom<StandardCompletion<X::Value>>,
    E: From<<C as TryLiftFrom<StandardCompletion<X::Value>>>::Error>,
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
        Ok(StatementEffect::Complete(C::try_lift_from(
            StandardCompletion::FunctionReturned(values),
        )?))
    }
}

impl<L, I, F, E, V, T> FunctionEntry<L, I, F, E, V> for Lexical<T>
where
    L: Dialect,
    Function<T>: FunctionEntry<L, I, F, E, V>,
    Lambda<T>: FunctionEntry<L, I, F, E, V>,
    E: LiftFrom<InterpreterError>,
    T: CompileTimeValue,
{
    fn enter_function_body(
        &self,
        location: Location,
        env: EnvIndex,
        interp: &mut I,
        args: Product<V>,
    ) -> Result<F, E> {
        match self {
            Lexical::Function(op) => op.enter_function_body(location, env, interp, args),
            Lexical::Lambda(op) => op.enter_function_body(location, env, interp, args),
            Lexical::Call(_) | Lexical::Return(_) => Err(E::lift_from(InterpreterError::Custom(
                "expected function body statement",
            ))),
        }
    }
}

impl<L, I, F, C, E, T, X> Interpretable<L, I, F, C, E, X> for Lexical<T>
where
    L: Dialect,
    Function<T>: Interpretable<L, I, F, C, E, X>,
    Lambda<T>: Interpretable<L, I, F, C, E, X>,
    Call<T>: Interpretable<L, I, F, C, E, X>,
    Return<T>: Interpretable<L, I, F, C, E, X>,
    T: CompileTimeValue,
    X: BlockTransfer,
{
    fn interpret(
        &self,
        location: Location,
        env: EnvIndex,
        interp: &mut I,
    ) -> Result<StatementEffect<F, C, X>, E> {
        match self {
            Lexical::Function(op) => <Function<T> as Interpretable<L, I, F, C, E, X>>::interpret(
                op, location, env, interp,
            ),
            Lexical::Lambda(op) => {
                <Lambda<T> as Interpretable<L, I, F, C, E, X>>::interpret(op, location, env, interp)
            }
            Lexical::Call(op) => {
                <Call<T> as Interpretable<L, I, F, C, E, X>>::interpret(op, location, env, interp)
            }
            Lexical::Return(op) => {
                <Return<T> as Interpretable<L, I, F, C, E, X>>::interpret(op, location, env, interp)
            }
        }
    }
}

impl<L, I, F, E, V, T> FunctionEntry<L, I, F, E, V> for Lifted<T>
where
    L: Dialect,
    Function<T>: FunctionEntry<L, I, F, E, V>,
    E: LiftFrom<InterpreterError>,
    T: CompileTimeValue,
{
    fn enter_function_body(
        &self,
        location: Location,
        env: EnvIndex,
        interp: &mut I,
        args: Product<V>,
    ) -> Result<F, E> {
        match self {
            Lifted::Function(op) => op.enter_function_body(location, env, interp, args),
            Lifted::Bind(_) | Lifted::Call(_) | Lifted::Return(_) => Err(E::lift_from(
                InterpreterError::Custom("expected function body statement"),
            )),
        }
    }
}

impl<L, I, F, C, E, T, X> Interpretable<L, I, F, C, E, X> for Lifted<T>
where
    L: Dialect,
    Function<T>: Interpretable<L, I, F, C, E, X>,
    Bind<T>: Interpretable<L, I, F, C, E, X>,
    Call<T>: Interpretable<L, I, F, C, E, X>,
    Return<T>: Interpretable<L, I, F, C, E, X>,
    T: CompileTimeValue,
    X: BlockTransfer,
{
    fn interpret(
        &self,
        location: Location,
        env: EnvIndex,
        interp: &mut I,
    ) -> Result<StatementEffect<F, C, X>, E> {
        match self {
            Lifted::Function(op) => <Function<T> as Interpretable<L, I, F, C, E, X>>::interpret(
                op, location, env, interp,
            ),
            Lifted::Bind(op) => {
                <Bind<T> as Interpretable<L, I, F, C, E, X>>::interpret(op, location, env, interp)
            }
            Lifted::Call(op) => {
                <Call<T> as Interpretable<L, I, F, C, E, X>>::interpret(op, location, env, interp)
            }
            Lifted::Return(op) => {
                <Return<T> as Interpretable<L, I, F, C, E, X>>::interpret(op, location, env, interp)
            }
        }
    }
}
