use std::ops::{Add, Mul, Neg, Sub};

use kirin::prelude::{CompileTimeValue, Dialect};
use kirin_interpreter::{Continuation, Interpretable, Interpreter, InterpreterError};

use crate::{Arith, CheckedDiv, CheckedRem};

#[derive(Debug)]
struct DivisionByZero;

impl std::fmt::Display for DivisionByZero {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "division by zero")
    }
}

impl std::error::Error for DivisionByZero {}

impl<'ir, I, L, T> Interpretable<'ir, I, L> for Arith<T>
where
    I: Interpreter<'ir>,
    I::Value: Clone
        + Add<Output = I::Value>
        + Sub<Output = I::Value>
        + Mul<Output = I::Value>
        + CheckedDiv
        + CheckedRem
        + Neg<Output = I::Value>,
    I::Error: From<InterpreterError>,
    L: Dialect,
    T: CompileTimeValue,
{
    fn interpret(&self, interp: &mut I) -> Result<Continuation<I::Value, I::Ext>, I::Error> {
        match self {
            Arith::Add {
                lhs, rhs, result, ..
            } => {
                let a = interp.read(*lhs)?;
                let b = interp.read(*rhs)?;
                interp.write(*result, a + b)?;
                Ok(Continuation::Continue)
            }
            Arith::Sub {
                lhs, rhs, result, ..
            } => {
                let a = interp.read(*lhs)?;
                let b = interp.read(*rhs)?;
                interp.write(*result, a - b)?;
                Ok(Continuation::Continue)
            }
            Arith::Mul {
                lhs, rhs, result, ..
            } => {
                let a = interp.read(*lhs)?;
                let b = interp.read(*rhs)?;
                interp.write(*result, a * b)?;
                Ok(Continuation::Continue)
            }
            Arith::Div {
                lhs, rhs, result, ..
            } => {
                let a = interp.read(*lhs)?;
                let b = interp.read(*rhs)?;
                let v = a
                    .checked_div(b)
                    .ok_or_else(|| InterpreterError::custom(DivisionByZero))?;
                interp.write(*result, v)?;
                Ok(Continuation::Continue)
            }
            Arith::Rem {
                lhs, rhs, result, ..
            } => {
                let a = interp.read(*lhs)?;
                let b = interp.read(*rhs)?;
                let v = a
                    .checked_rem(b)
                    .ok_or_else(|| InterpreterError::custom(DivisionByZero))?;
                interp.write(*result, v)?;
                Ok(Continuation::Continue)
            }
            Arith::Neg {
                operand, result, ..
            } => {
                let a = interp.read(*operand)?;
                interp.write(*result, -a)?;
                Ok(Continuation::Continue)
            }
        }
    }
}
