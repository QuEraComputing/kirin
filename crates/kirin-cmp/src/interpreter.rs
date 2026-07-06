use kirin::prelude::{CompileTimeValue, HasBottom};
use kirin_interpreter::dialect::{
    ClassicLiveness, ClassicLivenessInterp, DemandInterp, ForwardEval, Interpretable,
    SparseForwardEffect, SparseForwardInterp, StrongDemand,
};

use crate::{Cmp, CompareValue};

/// Classic (weak) per-point liveness: kill the result, gen all operands.
impl<I, T> Interpretable<I, ClassicLiveness> for Cmp<T>
where
    I: ClassicLivenessInterp,
    T: CompileTimeValue,
{
    fn interpret(&self, interp: &mut I) -> Result<I::Effect, I::Error> {
        interp.gen_uses_kill_defs(self)
    }
}

/// Backward demand: purity-aware neededness. `Cmp` is `#[kirin(pure)]`, so
/// operands are demanded only when a result is demanded.
impl<I, T> Interpretable<I, StrongDemand> for Cmp<T>
where
    I: DemandInterp,
    I::Value: HasBottom + PartialEq,
    T: CompileTimeValue,
{
    fn interpret(&self, interp: &mut I) -> Result<I::Effect, I::Error> {
        interp.demand_uses_if_observable(self)
    }
}

impl<I, T> Interpretable<I, ForwardEval> for Cmp<T>
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
