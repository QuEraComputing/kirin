use kirin::prelude::CompileTimeValue;
use kirin_interpreter_10::control::Control;
use kirin_interpreter_10::env::Env;
use kirin_interpreter_10::interpretable::Interpretable;

use crate::{Cmp, CompareValue};

fn eval_impl<E, T>(op: &Cmp<T>, env: &mut E) -> Result<(), E::Error>
where
    E: Env,
    E::Value: CompareValue,
    <E::Value as CompareValue>::Bool: Into<E::Value>,
    T: CompileTimeValue,
{
    match op {
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
        Cmp::__Phantom(..) => unreachable!(),
    }
    Ok(())
}

impl<E, T> Interpretable<E> for Cmp<T>
where
    E: Env,
    E::Value: CompareValue,
    <E::Value as CompareValue>::Bool: Into<E::Value>,
    T: CompileTimeValue,
{
    fn eval(&self, env: &mut E) -> Result<Control<E::Value, E::Ext>, E::Error> {
        eval_impl(self, env)?;
        Ok(Control::Advance)
    }
}
