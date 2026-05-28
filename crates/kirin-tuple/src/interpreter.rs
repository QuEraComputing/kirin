use kirin::prelude::{CompileTimeValue, Dialect, Product, SSAValue};
use kirin_interpreter::{
    BlockTransfer, Env, HasProductValue, Interpretable, InterpreterError, Location, StatementEffect,
};
use thiserror::Error;

use crate::{Get, Len, NewTuple, Unpack};

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
        env: kirin_interpreter::EnvIndex,
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
    E: From<ExpectedTuple> + From<InterpreterError>,
    T: CompileTimeValue,
{
    fn interpret(
        &self,
        _location: Location,
        env: kirin_interpreter::EnvIndex,
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
            .ok_or_else(|| E::from(ExpectedTuple))?
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
    E: From<ExpectedTuple> + From<InvalidTupleIndex> + From<TupleIndexOutOfBounds>,
    T: CompileTimeValue,
{
    fn interpret(
        &self,
        _location: Location,
        env: kirin_interpreter::EnvIndex,
        interp: &mut I,
    ) -> Result<StatementEffect<F, C, X>, E> {
        let source = interp.read(env, self.source)?;
        let index = interp
            .read(env, self.index)?
            .as_tuple_index()
            .ok_or_else(|| E::from(InvalidTupleIndex))?;
        let value = source
            .as_product()
            .ok_or_else(|| E::from(ExpectedTuple))?
            .get(index)
            .cloned()
            .ok_or_else(|| E::from(TupleIndexOutOfBounds))?;
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
    E: From<ExpectedTuple>,
    T: CompileTimeValue,
{
    fn interpret(
        &self,
        _location: Location,
        env: kirin_interpreter::EnvIndex,
        interp: &mut I,
    ) -> Result<StatementEffect<F, C, X>, E> {
        let source = interp.read(env, self.source)?;
        let len = source
            .as_product()
            .ok_or_else(|| E::from(ExpectedTuple))?
            .len();
        interp.write(
            env,
            SSAValue::from(self.result),
            X::Value::from_tuple_index(len),
        )?;
        Ok(StatementEffect::Done)
    }
}

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
#[error("expected tuple value")]
pub struct ExpectedTuple;

impl From<ExpectedTuple> for InterpreterError {
    fn from(_: ExpectedTuple) -> Self {
        Self::Custom("expected tuple value")
    }
}

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
#[error("invalid tuple index")]
pub struct InvalidTupleIndex;

impl From<InvalidTupleIndex> for InterpreterError {
    fn from(_: InvalidTupleIndex) -> Self {
        Self::Custom("invalid tuple index")
    }
}

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
#[error("tuple index out of bounds")]
pub struct TupleIndexOutOfBounds;

impl From<TupleIndexOutOfBounds> for InterpreterError {
    fn from(_: TupleIndexOutOfBounds) -> Self {
        Self::Custom("tuple index out of bounds")
    }
}

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
#[error("tuple arity mismatch")]
pub struct TupleArityMismatch;

impl From<TupleArityMismatch> for InterpreterError {
    fn from(_: TupleArityMismatch) -> Self {
        Self::Custom("tuple arity mismatch")
    }
}
