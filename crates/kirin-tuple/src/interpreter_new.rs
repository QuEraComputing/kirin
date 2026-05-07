use kirin::prelude::{CompileTimeValue, Dialect, LiftFrom, Product, SSAValue, TryLiftFrom};
use kirin_interpreter_new::{
    BlockTransfer, Env, HasProductValue, Interpretable, InterpreterError, Location, StatementEffect,
};
use thiserror::Error;

use crate::{Get, Len, NewTuple, Tuple, Unpack};

pub trait TupleIndexValue: Sized {
    fn as_tuple_index(&self) -> Option<usize>;
    fn from_tuple_index(index: usize) -> Self;
}

impl<L, I, F, C, E, T, X> Interpretable<L, I, F, C, E, X> for NewTuple<T>
where
    L: Dialect,
    I: Env<X::Value, Error = E>,
    X: BlockTransfer,
    X::Value: HasProductValue,
    T: CompileTimeValue,
{
    fn interpret(
        &self,
        _location: Location,
        env: kirin_interpreter_new::EnvIndex,
        interp: &mut I,
    ) -> Result<StatementEffect<F, C, X>, E> {
        let values = self
            .args
            .iter()
            .map(|arg| interp.read(env, *arg))
            .collect::<Result<Product<_>, _>>()?;
        interp.write(
            env,
            SSAValue::from(self.result),
            X::Value::from_product(values),
        )?;
        Ok(StatementEffect::Done)
    }
}

impl<L, I, F, C, E, T, X> Interpretable<L, I, F, C, E, X> for Unpack<T>
where
    L: Dialect,
    I: Env<X::Value, Error = E>,
    X: BlockTransfer,
    X::Value: HasProductValue,
    E: LiftFrom<ExpectedTuple> + LiftFrom<InterpreterError>,
    T: CompileTimeValue,
{
    fn interpret(
        &self,
        _location: Location,
        env: kirin_interpreter_new::EnvIndex,
        interp: &mut I,
    ) -> Result<StatementEffect<F, C, X>, E> {
        let source = interp.read(env, self.source)?;
        let results = self
            .results
            .iter()
            .copied()
            .map(SSAValue::from)
            .collect::<Vec<_>>();
        let product = source
            .as_product()
            .ok_or_else(|| E::lift_from(ExpectedTuple))?
            .clone();
        interp.write_product(env, results.as_slice(), product)?;
        Ok(StatementEffect::Done)
    }
}

impl<L, I, F, C, E, T, X> Interpretable<L, I, F, C, E, X> for Get<T>
where
    L: Dialect,
    I: Env<X::Value, Error = E>,
    X: BlockTransfer,
    X::Value: HasProductValue + TupleIndexValue,
    E: LiftFrom<ExpectedTuple> + LiftFrom<InvalidTupleIndex> + LiftFrom<TupleIndexOutOfBounds>,
    T: CompileTimeValue,
{
    fn interpret(
        &self,
        _location: Location,
        env: kirin_interpreter_new::EnvIndex,
        interp: &mut I,
    ) -> Result<StatementEffect<F, C, X>, E> {
        let source = interp.read(env, self.source)?;
        let index = interp
            .read(env, self.index)?
            .as_tuple_index()
            .ok_or_else(|| E::lift_from(InvalidTupleIndex))?;
        let value = source
            .as_product()
            .ok_or_else(|| E::lift_from(ExpectedTuple))?
            .get(index)
            .cloned()
            .ok_or_else(|| E::lift_from(TupleIndexOutOfBounds))?;
        interp.write(env, SSAValue::from(self.result), value)?;
        Ok(StatementEffect::Done)
    }
}

impl<L, I, F, C, E, T, X> Interpretable<L, I, F, C, E, X> for Len<T>
where
    L: Dialect,
    I: Env<X::Value, Error = E>,
    X: BlockTransfer,
    X::Value: HasProductValue + TupleIndexValue,
    E: LiftFrom<ExpectedTuple>,
    T: CompileTimeValue,
{
    fn interpret(
        &self,
        _location: Location,
        env: kirin_interpreter_new::EnvIndex,
        interp: &mut I,
    ) -> Result<StatementEffect<F, C, X>, E> {
        let source = interp.read(env, self.source)?;
        let len = source
            .as_product()
            .ok_or_else(|| E::lift_from(ExpectedTuple))?
            .len();
        interp.write(
            env,
            SSAValue::from(self.result),
            X::Value::from_tuple_index(len),
        )?;
        Ok(StatementEffect::Done)
    }
}

impl<L, I, F, C, E, T, X> Interpretable<L, I, F, C, E, X> for Tuple<T>
where
    L: Dialect,
    I: Env<X::Value, Error = E>,
    X: BlockTransfer,
    X::Value: HasProductValue + TupleIndexValue,
    E: LiftFrom<ExpectedTuple>
        + LiftFrom<InvalidTupleIndex>
        + LiftFrom<TupleIndexOutOfBounds>
        + LiftFrom<InterpreterError>,
    T: CompileTimeValue,
{
    fn interpret(
        &self,
        location: Location,
        env: kirin_interpreter_new::EnvIndex,
        interp: &mut I,
    ) -> Result<StatementEffect<F, C, X>, E> {
        match self {
            Tuple::NewTuple(op) => <NewTuple<T> as Interpretable<L, I, F, C, E, X>>::interpret(
                op, location, env, interp,
            ),
            Tuple::Unpack(op) => {
                <Unpack<T> as Interpretable<L, I, F, C, E, X>>::interpret(op, location, env, interp)
            }
            Tuple::Get(op) => {
                <Get<T> as Interpretable<L, I, F, C, E, X>>::interpret(op, location, env, interp)
            }
            Tuple::Len(op) => {
                <Len<T> as Interpretable<L, I, F, C, E, X>>::interpret(op, location, env, interp)
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
#[error("expected tuple value")]
pub struct ExpectedTuple;

impl TryLiftFrom<ExpectedTuple> for InterpreterError {
    type Error = core::convert::Infallible;

    fn try_lift_from(_: ExpectedTuple) -> Result<Self, Self::Error> {
        Ok(Self::Custom("expected tuple value"))
    }
}

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
#[error("invalid tuple index")]
pub struct InvalidTupleIndex;

impl TryLiftFrom<InvalidTupleIndex> for InterpreterError {
    type Error = core::convert::Infallible;

    fn try_lift_from(_: InvalidTupleIndex) -> Result<Self, Self::Error> {
        Ok(Self::Custom("invalid tuple index"))
    }
}

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
#[error("tuple index out of bounds")]
pub struct TupleIndexOutOfBounds;

impl TryLiftFrom<TupleIndexOutOfBounds> for InterpreterError {
    type Error = core::convert::Infallible;

    fn try_lift_from(_: TupleIndexOutOfBounds) -> Result<Self, Self::Error> {
        Ok(Self::Custom("tuple index out of bounds"))
    }
}

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
#[error("tuple arity mismatch")]
pub struct TupleArityMismatch;

impl TryLiftFrom<TupleArityMismatch> for InterpreterError {
    type Error = core::convert::Infallible;

    fn try_lift_from(_: TupleArityMismatch) -> Result<Self, Self::Error> {
        Ok(Self::Custom("tuple arity mismatch"))
    }
}
