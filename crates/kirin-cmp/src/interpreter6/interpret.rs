use kirin::prelude::CompileTimeValue;
use kirin_interpreter_6::env::{Env, Interpretable};
use kirin_interpreter_6::lift::Lift;

use crate::{Cmp, CompareValue};

impl<E, T> Interpretable<E> for Cmp<T>
where
    E: Env,
    E::Effect: Lift<()>,
    E::Value: CompareValue,
    <E::Value as CompareValue>::Bool: Into<E::Value>,
    T: CompileTimeValue,
{
    type DialectEffect = E::Effect;

    fn interpret(&self, env: &mut E) -> Result<E::Effect, E::Error> {
        match self {
            Cmp::Eq {
                lhs, rhs, result, ..
            } => {
                let lhs = env.read(*lhs)?;
                let rhs = env.read(*rhs)?;
                env.write(*result, lhs.cmp_eq(&rhs).into())?;
            }
            Cmp::Ne {
                lhs, rhs, result, ..
            } => {
                let lhs = env.read(*lhs)?;
                let rhs = env.read(*rhs)?;
                env.write(*result, lhs.cmp_ne(&rhs).into())?;
            }
            Cmp::Lt {
                lhs, rhs, result, ..
            } => {
                let lhs = env.read(*lhs)?;
                let rhs = env.read(*rhs)?;
                env.write(*result, lhs.cmp_lt(&rhs).into())?;
            }
            Cmp::Le {
                lhs, rhs, result, ..
            } => {
                let lhs = env.read(*lhs)?;
                let rhs = env.read(*rhs)?;
                env.write(*result, lhs.cmp_le(&rhs).into())?;
            }
            Cmp::Gt {
                lhs, rhs, result, ..
            } => {
                let lhs = env.read(*lhs)?;
                let rhs = env.read(*rhs)?;
                env.write(*result, lhs.cmp_gt(&rhs).into())?;
            }
            Cmp::Ge {
                lhs, rhs, result, ..
            } => {
                let lhs = env.read(*lhs)?;
                let rhs = env.read(*rhs)?;
                env.write(*result, lhs.cmp_ge(&rhs).into())?;
            }
            Self::__Phantom(..) => unreachable!(),
        }
        Ok(E::advance())
    }
}
