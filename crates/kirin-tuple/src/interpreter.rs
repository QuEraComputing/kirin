use kirin::prelude::{CompileTimeValue, Product};
use kirin_interpreter::InterpreterError;
use kirin_interpreter::dialect::{
    Ctx, ForwardEffect, ForwardInterp, HasProductValue, Interpretable,
};
use thiserror::Error;

use crate::{Get, Len, NewTuple, Unpack};

pub trait TupleIndexValue: Sized {
    fn as_tuple_index(&self) -> Option<usize>;
    fn from_tuple_index(index: usize) -> Self;
}

impl<I, T> Interpretable<I> for NewTuple<T>
where
    I: ForwardInterp,
    I::Value: HasProductValue,
    T: CompileTimeValue,
{
    fn interpret(&self, ctx: &mut Ctx<'_, I>) -> Result<I::Effect, I::Error> {
        let values = self
            .args
            .iter()
            .map(|arg| ctx.read(*arg))
            .collect::<Result<Product<_>, _>>()?;
        ctx.write(self.result, I::Value::from_product(values))?;
        Ok(ForwardEffect::Next)
    }
}

impl<I, T> Interpretable<I> for Unpack<T>
where
    I: ForwardInterp,
    I::Value: HasProductValue,
    I::Error: From<ExpectedTuple>,
    T: CompileTimeValue,
{
    fn interpret(&self, ctx: &mut Ctx<'_, I>) -> Result<I::Effect, I::Error> {
        let source = ctx.read(self.source)?;
        let product = source
            .as_product()
            .ok_or_else(|| I::Error::from(ExpectedTuple))?
            .clone();
        ctx.write_results(self.results.as_slice(), product)?;
        Ok(ForwardEffect::Next)
    }
}

impl<I, T> Interpretable<I> for Get<T>
where
    I: ForwardInterp,
    I::Value: HasProductValue + TupleIndexValue,
    I::Error: From<ExpectedTuple> + From<InvalidTupleIndex> + From<TupleIndexOutOfBounds>,
    T: CompileTimeValue,
{
    fn interpret(&self, ctx: &mut Ctx<'_, I>) -> Result<I::Effect, I::Error> {
        let source = ctx.read(self.source)?;
        let index = ctx
            .read(self.index)?
            .as_tuple_index()
            .ok_or_else(|| I::Error::from(InvalidTupleIndex))?;
        let value = source
            .as_product()
            .ok_or_else(|| I::Error::from(ExpectedTuple))?
            .get(index)
            .cloned()
            .ok_or_else(|| I::Error::from(TupleIndexOutOfBounds))?;
        ctx.write(self.result, value)?;
        Ok(ForwardEffect::Next)
    }
}

impl<I, T> Interpretable<I> for Len<T>
where
    I: ForwardInterp,
    I::Value: HasProductValue + TupleIndexValue,
    I::Error: From<ExpectedTuple>,
    T: CompileTimeValue,
{
    fn interpret(&self, ctx: &mut Ctx<'_, I>) -> Result<I::Effect, I::Error> {
        let source = ctx.read(self.source)?;
        let len = source
            .as_product()
            .ok_or_else(|| I::Error::from(ExpectedTuple))?
            .len();
        ctx.write(self.result, I::Value::from_tuple_index(len))?;
        Ok(ForwardEffect::Next)
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
