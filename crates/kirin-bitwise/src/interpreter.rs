use std::ops::{BitAnd, BitOr, BitXor, Not};

use kirin::prelude::{CompileTimeValue, HasBottom};
use kirin_interpreter::dialect::{
    DenseBackward, DenseBackwardInterp, Interpretable, InterpreterError, SparseBackward,
    SparseBackwardInterp, SparseForward, SparseForwardEffect, SparseForwardInterp,
};
use thiserror::Error;

use crate::{Bitwise, CheckedShl, CheckedShr};

/// Classic (weak) per-point liveness: kill the result, gen all operands.
impl<I, T> Interpretable<I, DenseBackward> for Bitwise<T>
where
    I: DenseBackwardInterp,
    T: CompileTimeValue,
{
    fn interpret(&self, interp: &mut I) -> Result<I::Effect, I::Error> {
        interp.transfer_classic(self)
    }
}

/// Backward demand: purity-aware neededness. `Bitwise` is `#[kirin(pure)]`, so
/// operands are demanded only when a result is demanded.
impl<I, T> Interpretable<I, SparseBackward> for Bitwise<T>
where
    I: SparseBackwardInterp,
    I::Value: HasBottom + PartialEq,
    T: CompileTimeValue,
{
    fn interpret(&self, interp: &mut I) -> Result<I::Effect, I::Error> {
        interp.transfer_ordinary(self)
    }
}

impl<I, T> Interpretable<I, SparseForward> for Bitwise<T>
where
    I: SparseForwardInterp,
    I::Value: BitAnd<Output = I::Value>
        + BitOr<Output = I::Value>
        + BitXor<Output = I::Value>
        + Not<Output = I::Value>
        + CheckedShl
        + CheckedShr,
    I::Error: From<ShiftOverflow>,
    T: CompileTimeValue,
{
    fn interpret(&self, interp: &mut I) -> Result<I::Effect, I::Error> {
        match self {
            Bitwise::And {
                lhs, rhs, result, ..
            } => {
                let value = interp.read(*lhs)? & interp.read(*rhs)?;
                interp.write(*result, value)?;
            }
            Bitwise::Or {
                lhs, rhs, result, ..
            } => {
                let value = interp.read(*lhs)? | interp.read(*rhs)?;
                interp.write(*result, value)?;
            }
            Bitwise::Xor {
                lhs, rhs, result, ..
            } => {
                let value = interp.read(*lhs)? ^ interp.read(*rhs)?;
                interp.write(*result, value)?;
            }
            Bitwise::Not {
                operand, result, ..
            } => {
                let value = !interp.read(*operand)?;
                interp.write(*result, value)?;
            }
            Bitwise::Shl {
                lhs, rhs, result, ..
            } => {
                let value = interp
                    .read(*lhs)?
                    .checked_shl(interp.read(*rhs)?)
                    .ok_or_else(|| I::Error::from(ShiftOverflow))?;
                interp.write(*result, value)?;
            }
            Bitwise::Shr {
                lhs, rhs, result, ..
            } => {
                let value = interp
                    .read(*lhs)?
                    .checked_shr(interp.read(*rhs)?)
                    .ok_or_else(|| I::Error::from(ShiftOverflow))?;
                interp.write(*result, value)?;
            }
            Bitwise::__Phantom(..) => unreachable!(),
        }
        Ok(SparseForwardEffect::Next)
    }
}

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
#[error("shift overflow")]
pub struct ShiftOverflow;

impl From<ShiftOverflow> for InterpreterError {
    fn from(_: ShiftOverflow) -> Self {
        Self::Custom("shift overflow")
    }
}
