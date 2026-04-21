use kirin::prelude::CompileTimeValue;
use kirin_interpreter_20::control::Control;
use kirin_interpreter_20::env::Env;
use kirin_interpreter_20::interpretable::Interpretable;

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
            env.write_result(*result, env.read(*lhs)?.cmp_eq(&env.read(*rhs)?).into())?;
        }
        Cmp::Ne {
            lhs, rhs, result, ..
        } => {
            env.write_result(*result, env.read(*lhs)?.cmp_ne(&env.read(*rhs)?).into())?;
        }
        Cmp::Lt {
            lhs, rhs, result, ..
        } => {
            env.write_result(*result, env.read(*lhs)?.cmp_lt(&env.read(*rhs)?).into())?;
        }
        Cmp::Le {
            lhs, rhs, result, ..
        } => {
            env.write_result(*result, env.read(*lhs)?.cmp_le(&env.read(*rhs)?).into())?;
        }
        Cmp::Gt {
            lhs, rhs, result, ..
        } => {
            env.write_result(*result, env.read(*lhs)?.cmp_gt(&env.read(*rhs)?).into())?;
        }
        Cmp::Ge {
            lhs, rhs, result, ..
        } => {
            env.write_result(*result, env.read(*lhs)?.cmp_ge(&env.read(*rhs)?).into())?;
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
