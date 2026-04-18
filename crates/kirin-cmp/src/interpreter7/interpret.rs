use kirin::prelude::CompileTimeValue;
use kirin_interpreter_7::env::{Interp, Interpretable};

use crate::{Cmp, CompareValue};

/// Pure value op: returns `()` (advance).
impl<E, T> Interpretable<E> for Cmp<T>
where
    E: Interp,
    E::Value: CompareValue,
    <E::Value as CompareValue>::Bool: Into<E::Value>,
    T: CompileTimeValue,
{
    type Effect = ();

    fn interpret(&self, env: &mut E) -> Result<(), E::Error> {
        match self {
            Cmp::Eq {
                lhs, rhs, result, ..
            } => {
                let lhs = env.read(*lhs)?;
                let rhs = env.read(*rhs)?;
                env.write_result(*result, lhs.cmp_eq(&rhs).into())?;
            }
            Cmp::Ne {
                lhs, rhs, result, ..
            } => {
                let lhs = env.read(*lhs)?;
                let rhs = env.read(*rhs)?;
                env.write_result(*result, lhs.cmp_ne(&rhs).into())?;
            }
            Cmp::Lt {
                lhs, rhs, result, ..
            } => {
                let lhs = env.read(*lhs)?;
                let rhs = env.read(*rhs)?;
                env.write_result(*result, lhs.cmp_lt(&rhs).into())?;
            }
            Cmp::Le {
                lhs, rhs, result, ..
            } => {
                let lhs = env.read(*lhs)?;
                let rhs = env.read(*rhs)?;
                env.write_result(*result, lhs.cmp_le(&rhs).into())?;
            }
            Cmp::Gt {
                lhs, rhs, result, ..
            } => {
                let lhs = env.read(*lhs)?;
                let rhs = env.read(*rhs)?;
                env.write_result(*result, lhs.cmp_gt(&rhs).into())?;
            }
            Cmp::Ge {
                lhs, rhs, result, ..
            } => {
                let lhs = env.read(*lhs)?;
                let rhs = env.read(*rhs)?;
                env.write_result(*result, lhs.cmp_ge(&rhs).into())?;
            }
            Self::__Phantom(..) => unreachable!(),
        }
        Ok(())
    }
}
