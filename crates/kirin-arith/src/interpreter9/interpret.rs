use std::ops::{Add, Mul, Neg, Sub};

use kirin::prelude::CompileTimeValue;
use kirin_interpreter_9::control::Control;
use kirin_interpreter_9::env::Env;
use kirin_interpreter_9::error::InterpreterError;
use kirin_interpreter_9::interpretable::Interpretable;

use crate::{Arith, CheckedDiv, CheckedRem};

fn eval_impl<E, T>(op: &Arith<T>, env: &mut E) -> Result<(), E::Error>
where
    E: Env,
    E::Value: Clone
        + Add<Output = E::Value>
        + Sub<Output = E::Value>
        + Mul<Output = E::Value>
        + Neg<Output = E::Value>
        + CheckedDiv
        + CheckedRem,
    E::Error: From<InterpreterError>,
    T: CompileTimeValue,
{
    match op {
        Arith::Add {
            lhs, rhs, result, ..
        } => {
            let l = env.read(*lhs)?;
            let r = env.read(*rhs)?;
            env.write_result(*result, l + r)?;
        }
        Arith::Sub {
            lhs, rhs, result, ..
        } => {
            let l = env.read(*lhs)?;
            let r = env.read(*rhs)?;
            env.write_result(*result, l - r)?;
        }
        Arith::Mul {
            lhs, rhs, result, ..
        } => {
            let l = env.read(*lhs)?;
            let r = env.read(*rhs)?;
            env.write_result(*result, l * r)?;
        }
        Arith::Div {
            lhs, rhs, result, ..
        } => {
            let l = env.read(*lhs)?;
            let r = env.read(*rhs)?;
            let v = l.checked_div(r).ok_or_else(|| {
                E::Error::from(InterpreterError::Custom(Box::new(DivisionByZero)))
            })?;
            env.write_result(*result, v)?;
        }
        Arith::Rem {
            lhs, rhs, result, ..
        } => {
            let l = env.read(*lhs)?;
            let r = env.read(*rhs)?;
            let v = l.checked_rem(r).ok_or_else(|| {
                E::Error::from(InterpreterError::Custom(Box::new(DivisionByZero)))
            })?;
            env.write_result(*result, v)?;
        }
        Arith::Neg {
            operand, result, ..
        } => {
            let v = env.read(*operand)?;
            env.write_result(*result, -v)?;
        }
        Arith::__Phantom(..) => unreachable!(),
    }
    Ok(())
}

/// Single generic impl for all interpreter modes.
impl<E, T> Interpretable<E> for Arith<T>
where
    E: Env,
    E::Value: Clone
        + Add<Output = E::Value>
        + Sub<Output = E::Value>
        + Mul<Output = E::Value>
        + Neg<Output = E::Value>
        + CheckedDiv
        + CheckedRem,
    E::Error: From<InterpreterError>,
    T: CompileTimeValue,
{
    fn eval(&self, env: &mut E) -> Result<Control<E::Value, E::Ext>, E::Error> {
        eval_impl(self, env)?;
        Ok(Control::Advance)
    }
}

#[derive(Debug)]
struct DivisionByZero;

impl std::fmt::Display for DivisionByZero {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "division by zero")
    }
}

impl std::error::Error for DivisionByZero {}
