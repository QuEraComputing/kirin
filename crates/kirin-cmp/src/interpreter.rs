use kirin::prelude::CompileTimeValue;
use kirin_interpreter::dialect::{ForwardEffect, ForwardEval, ForwardEvalInterp, Interpretable};

use crate::{Cmp, CompareValue};

impl<I, T> Interpretable<I, ForwardEval> for Cmp<T>
where
    I: ForwardEvalInterp,
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
        Ok(ForwardEffect::Next)
    }
}
