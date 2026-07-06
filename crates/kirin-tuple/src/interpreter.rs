use kirin::prelude::{CompileTimeValue, HasBottom, Product};
use kirin_interpreter::InterpreterError;
use kirin_interpreter::dialect::{
    DenseBackward, DenseBackwardInterp, HasProductValue, Interpretable, SparseBackward,
    SparseBackwardInterp, SparseForward, SparseForwardEffect, SparseForwardInterp,
};
use thiserror::Error;

use crate::{Get, Len, NewTuple, Unpack};

/// Backward rules for the tuple ops. Sparse: purity-aware neededness (all
/// four are `#[kirin(pure)]` data ops, so operands are demanded only when a
/// result is demanded). Dense: the classic kill-defs/gen-uses transfer.
macro_rules! backward_ordinary {
    ($ty:ident) => {
        impl<I, T> Interpretable<I, SparseBackward> for $ty<T>
        where
            I: SparseBackwardInterp,
            I::Value: HasBottom + PartialEq,
            T: CompileTimeValue,
        {
            fn interpret(&self, interp: &mut I) -> Result<I::Effect, I::Error> {
                interp.transfer_ordinary(self)
            }
        }

        impl<I, T> Interpretable<I, DenseBackward> for $ty<T>
        where
            I: DenseBackwardInterp,
            T: CompileTimeValue,
        {
            fn interpret(&self, interp: &mut I) -> Result<I::Effect, I::Error> {
                interp.transfer_classic(self)
            }
        }
    };
}

backward_ordinary!(NewTuple);
backward_ordinary!(Unpack);
backward_ordinary!(Get);
backward_ordinary!(Len);

pub trait TupleIndexValue: Sized {
    fn as_tuple_index(&self) -> Option<usize>;
    fn from_tuple_index(index: usize) -> Self;
}

impl<I, T> Interpretable<I, SparseForward> for NewTuple<T>
where
    I: SparseForwardInterp,
    I::Value: HasProductValue,
    T: CompileTimeValue,
{
    fn interpret(&self, interp: &mut I) -> Result<I::Effect, I::Error> {
        let values = self
            .args
            .iter()
            .map(|arg| interp.read(*arg))
            .collect::<Result<Product<_>, _>>()?;
        interp.write(self.result, I::Value::from_product(values))?;
        Ok(SparseForwardEffect::Next)
    }
}

impl<I, T> Interpretable<I, SparseForward> for Unpack<T>
where
    I: SparseForwardInterp,
    I::Value: HasProductValue,
    I::Error: From<ExpectedTuple>,
    T: CompileTimeValue,
{
    fn interpret(&self, interp: &mut I) -> Result<I::Effect, I::Error> {
        let source = interp.read(self.source)?;
        let product = source
            .as_product()
            .ok_or_else(|| I::Error::from(ExpectedTuple))?
            .clone();
        interp.write_results(self.results.as_slice(), product)?;
        Ok(SparseForwardEffect::Next)
    }
}

impl<I, T> Interpretable<I, SparseForward> for Get<T>
where
    I: SparseForwardInterp,
    I::Value: HasProductValue + TupleIndexValue,
    I::Error: From<ExpectedTuple> + From<InvalidTupleIndex> + From<TupleIndexOutOfBounds>,
    T: CompileTimeValue,
{
    fn interpret(&self, interp: &mut I) -> Result<I::Effect, I::Error> {
        let source = interp.read(self.source)?;
        let index = interp
            .read(self.index)?
            .as_tuple_index()
            .ok_or_else(|| I::Error::from(InvalidTupleIndex))?;
        let value = source
            .as_product()
            .ok_or_else(|| I::Error::from(ExpectedTuple))?
            .get(index)
            .cloned()
            .ok_or_else(|| I::Error::from(TupleIndexOutOfBounds))?;
        interp.write(self.result, value)?;
        Ok(SparseForwardEffect::Next)
    }
}

impl<I, T> Interpretable<I, SparseForward> for Len<T>
where
    I: SparseForwardInterp,
    I::Value: HasProductValue + TupleIndexValue,
    I::Error: From<ExpectedTuple>,
    T: CompileTimeValue,
{
    fn interpret(&self, interp: &mut I) -> Result<I::Effect, I::Error> {
        let source = interp.read(self.source)?;
        let len = source
            .as_product()
            .ok_or_else(|| I::Error::from(ExpectedTuple))?
            .len();
        interp.write(self.result, I::Value::from_tuple_index(len))?;
        Ok(SparseForwardEffect::Next)
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
