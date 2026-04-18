use std::ops::{Add, Mul, Neg, Sub};

use kirin::prelude::{CompileTimeValue, Dialect, HasStageInfo, StageMeta};
use kirin_interpreter::AbstractValue;
use kirin_interpreter_7::abstract_interp::AbstractInterp;
use kirin_interpreter_7::concrete::ConcreteInterp;
use kirin_interpreter_7::env::Interpretable;
use kirin_interpreter_7::error::InterpreterError;
use kirin_interpreter_7::store::Store;

use crate::{Arith, CheckedDiv, CheckedRem};

#[derive(Debug)]
struct DivisionByZero;

impl std::fmt::Display for DivisionByZero {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "division by zero")
    }
}

impl std::error::Error for DivisionByZero {}

fn interp_impl<S, T>(op: &Arith<T>, env: &mut S) -> Result<(), S::Error>
where
    S: Store,
    S::Value: Clone
        + Add<Output = S::Value>
        + Sub<Output = S::Value>
        + Mul<Output = S::Value>
        + Neg<Output = S::Value>
        + CheckedDiv
        + CheckedRem,
    S::Error: From<InterpreterError>,
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
                S::Error::from(InterpreterError::Custom(Box::new(DivisionByZero)))
            })?;
            env.write_result(*result, v)?;
        }
        Arith::Rem {
            lhs, rhs, result, ..
        } => {
            let l = env.read(*lhs)?;
            let r = env.read(*rhs)?;
            let v = l.checked_rem(r).ok_or_else(|| {
                S::Error::from(InterpreterError::Custom(Box::new(DivisionByZero)))
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

impl<'ir, S, L, V, C, T> Interpretable<ConcreteInterp<'ir, S, L, V, C>> for Arith<T>
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
    V: Clone
        + Add<Output = V>
        + Sub<Output = V>
        + Mul<Output = V>
        + Neg<Output = V>
        + CheckedDiv
        + CheckedRem,
    C: 'static,
    T: CompileTimeValue,
{
    type Effect = ();

    fn interpret(&self, env: &mut ConcreteInterp<'ir, S, L, V, C>) -> Result<(), InterpreterError> {
        interp_impl(self, env)
    }
}

impl<'ir, S, L, V, T> Interpretable<AbstractInterp<'ir, S, L, V>> for Arith<T>
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
    V: Clone
        + AbstractValue
        + Add<Output = V>
        + Sub<Output = V>
        + Mul<Output = V>
        + Neg<Output = V>
        + CheckedDiv
        + CheckedRem,
    T: CompileTimeValue,
{
    type Effect = ();

    fn interpret(&self, env: &mut AbstractInterp<'ir, S, L, V>) -> Result<(), InterpreterError> {
        interp_impl(self, env)
    }
}
