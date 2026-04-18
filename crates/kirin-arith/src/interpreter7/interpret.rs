use std::ops::{Add, Mul, Neg, Sub};

use kirin::prelude::CompileTimeValue;
use kirin_interpreter_7::env::{Interp, Interpretable};
use kirin_interpreter_7::error::InterpreterError;

use crate::{Arith, CheckedDiv, CheckedRem};

#[derive(Debug)]
struct DivisionByZero;

impl std::fmt::Display for DivisionByZero {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "division by zero")
    }
}

impl std::error::Error for DivisionByZero {}

/// Pure value op: returns `()` (advance).
///
/// The dialect wrapper converts `()` to `Control::Advance` via
/// `op.interpret(env).map(Control::from)`.
impl<E, T> Interpretable<E> for Arith<T>
where
    E: Interp,
    E::Value: Clone
        + Add<Output = E::Value>
        + Sub<Output = E::Value>
        + Mul<Output = E::Value>
        + Neg<Output = E::Value>
        + CheckedDiv
        + CheckedRem,
    T: CompileTimeValue,
{
    type Effect = ();

    fn interpret(&self, env: &mut E) -> Result<(), E::Error> {
        match self {
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
            Self::__Phantom(..) => unreachable!(),
        }
        Ok(())
    }
}
