use std::hash::Hash;

use kirin::ir::TryLiftFrom;
use kirin::prelude::{
    CompileTimeValue, Dialect, Function, HasRegionBody, HasStageInfo, SSAValue, Symbol,
};
use kirin_interpreter_new::{
    AbstractInterpreter, BlockTransfer, CallFrame, Callee, ConcreteInterpreter, Env, EnvIndex,
    FunctionBodyEntry, Interpretable, InterpreterError, Location, ProductValue, RegionFrame,
    SimpleFixpointInterpreter, StageAccess, StandardCompletion, StatementEffect, Summary,
};

use crate::{Bind, Call, FunctionBody, Lambda, Lexical, Lifted, Return};

pub trait CallTargetResolution<L: Dialect> {
    type Error;

    fn resolve_call_target(
        &self,
        location: Location,
        target: Symbol,
    ) -> Result<Function, Self::Error>;
}

impl<'ir, S, L, F, C, E, V> CallTargetResolution<L> for ConcreteInterpreter<'ir, S, F, C, E, V>
where
    S: HasStageInfo<L>,
    L: Dialect,
    E: From<InterpreterError>,
{
    type Error = E;

    fn resolve_call_target(
        &self,
        location: Location,
        target: Symbol,
    ) -> Result<Function, Self::Error> {
        let stage = StageAccess::<L>::stage_info(self, location.stage)?;
        self.pipeline()
            .resolve_function(stage, target)
            .ok_or(InterpreterError::MissingCallTarget { location, target })
            .map_err(E::from)
    }
}

impl<'ir, S, L, F, C, E, V> CallTargetResolution<L> for AbstractInterpreter<'ir, S, F, C, E, V>
where
    S: HasStageInfo<L>,
    L: Dialect,
    E: From<InterpreterError>,
{
    type Error = E;

    fn resolve_call_target(
        &self,
        location: Location,
        target: Symbol,
    ) -> Result<Function, Self::Error> {
        let stage = StageAccess::<L>::stage_info(self, location.stage)?;
        self.pipeline()
            .resolve_function(stage, target)
            .ok_or(InterpreterError::MissingCallTarget { location, target })
            .map_err(E::from)
    }
}

impl<'ir, S, K, L, F, C, E, Sum, Store> CallTargetResolution<L>
    for SimpleFixpointInterpreter<'ir, S, K, F, C, E, Sum, Store>
where
    S: HasStageInfo<L>,
    K: Clone + Eq + Hash,
    L: Dialect,
    Sum: Summary,
    E: From<InterpreterError>,
{
    type Error = E;

    fn resolve_call_target(
        &self,
        location: Location,
        target: Symbol,
    ) -> Result<Function, Self::Error> {
        let stage = StageAccess::<L>::stage_info(self, location.stage)?;
        self.pipeline()
            .resolve_function(stage, target)
            .ok_or(InterpreterError::MissingCallTarget { location, target })
            .map_err(E::from)
    }
}

impl<L, I, F, E, V, T> FunctionBodyEntry<L, I, F, E, V> for FunctionBody<T>
where
    L: Dialect,
    F: From<RegionFrame<L, V>>,
    T: CompileTimeValue,
    V: Clone,
{
    fn enter_function_body(
        &self,
        location: Location,
        env: EnvIndex,
        _interp: &mut I,
        args: Vec<V>,
    ) -> Result<F, E> {
        Ok(RegionFrame::<L, V>::new(location.stage, *self.region(), env, args).into())
    }
}

impl<L, I, F, E, V, T> FunctionBodyEntry<L, I, F, E, V> for Lambda<T>
where
    L: Dialect,
    F: From<RegionFrame<L, V>>,
    T: CompileTimeValue,
    V: Clone,
{
    fn enter_function_body(
        &self,
        location: Location,
        env: EnvIndex,
        _interp: &mut I,
        args: Vec<V>,
    ) -> Result<F, E> {
        Ok(RegionFrame::<L, V>::new(location.stage, *self.region(), env, args).into())
    }
}

impl<L, I, F, C, E, V, T> Interpretable<L, I, F, C, E, BlockTransfer<V>> for FunctionBody<T>
where
    L: Dialect,
    F: From<RegionFrame<L, V>>,
    T: CompileTimeValue,
    V: Clone,
{
    fn interpret(
        &self,
        location: Location,
        env: EnvIndex,
        _interp: &mut I,
    ) -> Result<StatementEffect<F, C, BlockTransfer<V>>, E> {
        Ok(StatementEffect::Push(
            RegionFrame::<L, V>::new(location.stage, *self.region(), env, Vec::new()).into(),
        ))
    }
}

impl<L, I, F, C, E, V, T> Interpretable<L, I, F, C, E, BlockTransfer<V>> for Lambda<T>
where
    L: Dialect,
    F: From<RegionFrame<L, V>>,
    T: CompileTimeValue,
    V: Clone,
{
    fn interpret(
        &self,
        location: Location,
        env: EnvIndex,
        _interp: &mut I,
    ) -> Result<StatementEffect<F, C, BlockTransfer<V>>, E> {
        Ok(StatementEffect::Push(
            RegionFrame::<L, V>::new(location.stage, *self.region(), env, Vec::new()).into(),
        ))
    }
}

impl<L, I, F, C, E, V, T> Interpretable<L, I, F, C, E, BlockTransfer<V>> for Bind<T>
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
    ) -> Result<StatementEffect<F, C, BlockTransfer<V>>, E> {
        let _ = self;
        Err(InterpreterError::Custom("bind is not yet supported in the new interpreter").into())
    }
}

impl<L, I, F, C, E, V, T> Interpretable<L, I, F, C, E, BlockTransfer<V>> for Call<T>
where
    L: Dialect,
    I: CallTargetResolution<L, Error = E>,
    F: From<CallFrame<L, V>>,
    T: CompileTimeValue,
{
    fn interpret(
        &self,
        location: Location,
        env: EnvIndex,
        interp: &mut I,
    ) -> Result<StatementEffect<F, C, BlockTransfer<V>>, E> {
        let function = interp.resolve_call_target(location, self.target())?;
        let args = self.args().to_vec();
        let results = self.results().iter().copied().map(SSAValue::from).collect();
        Ok(StatementEffect::Push(
            CallFrame::<L, V>::new(location, Callee::Function(function), args, env, results).into(),
        ))
    }
}

impl<L, I, F, C, E, V, T> Interpretable<L, I, F, C, E, BlockTransfer<V>> for Return<T>
where
    L: Dialect,
    I: Env<V, Error = E>,
    C: TryLiftFrom<StandardCompletion<V>>,
    E: From<<C as TryLiftFrom<StandardCompletion<V>>>::Error>,
    T: CompileTimeValue,
    V: ProductValue,
{
    fn interpret(
        &self,
        _location: Location,
        env: EnvIndex,
        interp: &mut I,
    ) -> Result<StatementEffect<F, C, BlockTransfer<V>>, E> {
        let values = interp.read_many(env, self.values.as_slice())?;
        Ok(StatementEffect::Complete(C::try_lift_from(
            StandardCompletion::FunctionReturned(V::new_product(values)),
        )?))
    }
}

impl<L, I, F, E, V, T> FunctionBodyEntry<L, I, F, E, V> for Lexical<T>
where
    L: Dialect,
    FunctionBody<T>: FunctionBodyEntry<L, I, F, E, V>,
    Lambda<T>: FunctionBodyEntry<L, I, F, E, V>,
    E: From<InterpreterError>,
    T: CompileTimeValue,
{
    fn enter_function_body(
        &self,
        location: Location,
        env: EnvIndex,
        interp: &mut I,
        args: Vec<V>,
    ) -> Result<F, E> {
        match self {
            Lexical::FunctionBody(op) => op.enter_function_body(location, env, interp, args),
            Lexical::Lambda(op) => op.enter_function_body(location, env, interp, args),
            Lexical::Call(_) | Lexical::Return(_) => {
                Err(InterpreterError::Custom("expected function body statement").into())
            }
        }
    }
}

impl<L, I, F, C, E, V, T> Interpretable<L, I, F, C, E, BlockTransfer<V>> for Lexical<T>
where
    L: Dialect,
    FunctionBody<T>: Interpretable<L, I, F, C, E, BlockTransfer<V>>,
    Lambda<T>: Interpretable<L, I, F, C, E, BlockTransfer<V>>,
    Call<T>: Interpretable<L, I, F, C, E, BlockTransfer<V>>,
    Return<T>: Interpretable<L, I, F, C, E, BlockTransfer<V>>,
    T: CompileTimeValue,
{
    fn interpret(
        &self,
        location: Location,
        env: EnvIndex,
        interp: &mut I,
    ) -> Result<StatementEffect<F, C, BlockTransfer<V>>, E> {
        match self {
            Lexical::FunctionBody(op) => {
                <FunctionBody<T> as Interpretable<L, I, F, C, E, BlockTransfer<V>>>::interpret(
                    op, location, env, interp,
                )
            }
            Lexical::Lambda(op) => {
                <Lambda<T> as Interpretable<L, I, F, C, E, BlockTransfer<V>>>::interpret(
                    op, location, env, interp,
                )
            }
            Lexical::Call(op) => {
                <Call<T> as Interpretable<L, I, F, C, E, BlockTransfer<V>>>::interpret(
                    op, location, env, interp,
                )
            }
            Lexical::Return(op) => {
                <Return<T> as Interpretable<L, I, F, C, E, BlockTransfer<V>>>::interpret(
                    op, location, env, interp,
                )
            }
        }
    }
}

impl<L, I, F, E, V, T> FunctionBodyEntry<L, I, F, E, V> for Lifted<T>
where
    L: Dialect,
    FunctionBody<T>: FunctionBodyEntry<L, I, F, E, V>,
    E: From<InterpreterError>,
    T: CompileTimeValue,
{
    fn enter_function_body(
        &self,
        location: Location,
        env: EnvIndex,
        interp: &mut I,
        args: Vec<V>,
    ) -> Result<F, E> {
        match self {
            Lifted::FunctionBody(op) => op.enter_function_body(location, env, interp, args),
            Lifted::Bind(_) | Lifted::Call(_) | Lifted::Return(_) => {
                Err(InterpreterError::Custom("expected function body statement").into())
            }
        }
    }
}

impl<L, I, F, C, E, V, T> Interpretable<L, I, F, C, E, BlockTransfer<V>> for Lifted<T>
where
    L: Dialect,
    FunctionBody<T>: Interpretable<L, I, F, C, E, BlockTransfer<V>>,
    Bind<T>: Interpretable<L, I, F, C, E, BlockTransfer<V>>,
    Call<T>: Interpretable<L, I, F, C, E, BlockTransfer<V>>,
    Return<T>: Interpretable<L, I, F, C, E, BlockTransfer<V>>,
    T: CompileTimeValue,
{
    fn interpret(
        &self,
        location: Location,
        env: EnvIndex,
        interp: &mut I,
    ) -> Result<StatementEffect<F, C, BlockTransfer<V>>, E> {
        match self {
            Lifted::FunctionBody(op) => {
                <FunctionBody<T> as Interpretable<L, I, F, C, E, BlockTransfer<V>>>::interpret(
                    op, location, env, interp,
                )
            }
            Lifted::Bind(op) => {
                <Bind<T> as Interpretable<L, I, F, C, E, BlockTransfer<V>>>::interpret(
                    op, location, env, interp,
                )
            }
            Lifted::Call(op) => {
                <Call<T> as Interpretable<L, I, F, C, E, BlockTransfer<V>>>::interpret(
                    op, location, env, interp,
                )
            }
            Lifted::Return(op) => {
                <Return<T> as Interpretable<L, I, F, C, E, BlockTransfer<V>>>::interpret(
                    op, location, env, interp,
                )
            }
        }
    }
}
