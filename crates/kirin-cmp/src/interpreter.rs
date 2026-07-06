use kirin::prelude::{CompileTimeValue, HasBottom};
use kirin_interpreter::dialect::{
    DenseBackward, DenseBackwardInterp, Interpretable, SparseBackward, SparseBackwardInterp,
    SparseForward, SparseForwardEffect, SparseForwardInterp,
};

use crate::{Cmp, CompareValue};

/// Classic (weak) per-point liveness: kill the result, gen all operands.
impl<I, T> Interpretable<I, DenseBackward> for Cmp<T>
where
    I: DenseBackwardInterp,
    T: CompileTimeValue,
{
    fn interpret(&self, interp: &mut I) -> Result<I::Effect, I::Error> {
        interp.transfer_classic(self)
    }
}

/// Backward demand: purity-aware neededness. `Cmp` is `#[kirin(pure)]`, so
/// operands are demanded only when a result is demanded.
impl<I, T> Interpretable<I, SparseBackward> for Cmp<T>
where
    I: SparseBackwardInterp,
    I::Value: HasBottom + PartialEq,
    T: CompileTimeValue,
{
    fn interpret(&self, interp: &mut I) -> Result<I::Effect, I::Error> {
        interp.transfer_ordinary(self)
    }
}

impl<I, T> Interpretable<I, SparseForward> for Cmp<T>
where
    I: SparseForwardInterp,
    I::Value: CompareValue,
    <I::Value as CompareValue>::Bool: Into<I::Value>,
    T: CompileTimeValue,
{
    fn interpret(&self, interp: &mut I) -> Result<I::Effect, I::Error> {
        match self {
            Cmp::Eq {
                lhs, rhs, result, ..
            } => {
                let value = interp.read(*lhs)?.cmp_eq(&interp.read(*rhs)?).into();
                interp.write(*result, value)?;
            }
            Cmp::Ne {
                lhs, rhs, result, ..
            } => {
                let value = interp.read(*lhs)?.cmp_ne(&interp.read(*rhs)?).into();
                interp.write(*result, value)?;
            }
            Cmp::Lt {
                lhs, rhs, result, ..
            } => {
                let value = interp.read(*lhs)?.cmp_lt(&interp.read(*rhs)?).into();
                interp.write(*result, value)?;
            }
            Cmp::Le {
                lhs, rhs, result, ..
            } => {
                let value = interp.read(*lhs)?.cmp_le(&interp.read(*rhs)?).into();
                interp.write(*result, value)?;
            }
            Cmp::Gt {
                lhs, rhs, result, ..
            } => {
                let value = interp.read(*lhs)?.cmp_gt(&interp.read(*rhs)?).into();
                interp.write(*result, value)?;
            }
            Cmp::Ge {
                lhs, rhs, result, ..
            } => {
                let value = interp.read(*lhs)?.cmp_ge(&interp.read(*rhs)?).into();
                interp.write(*result, value)?;
            }
            Cmp::__Phantom(..) => unreachable!(),
        }
        Ok(SparseForwardEffect::Next)
    }
}
