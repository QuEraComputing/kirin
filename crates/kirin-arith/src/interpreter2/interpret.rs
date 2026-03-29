use std::ops::{Add, Mul, Neg, Sub};

use kirin::prelude::CompileTimeValue;
use kirin_interpreter_2::{Interpretable, Interpreter, InterpreterError, effect::Cursor};

use crate::{Arith, CheckedDiv, CheckedRem};

#[derive(Debug)]
struct DivisionByZero;

impl std::fmt::Display for DivisionByZero {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "division by zero")
    }
}

impl std::error::Error for DivisionByZero {}

impl<'ir, I, T> Interpretable<'ir, I> for Arith<T>
where
    I: Interpreter<'ir>,
    I::Value: Clone
        + Add<Output = I::Value>
        + Sub<Output = I::Value>
        + Mul<Output = I::Value>
        + CheckedDiv
        + CheckedRem
        + Neg<Output = I::Value>,
    T: CompileTimeValue,
{
    type Effect = Cursor;
    type Error = I::Error;

    fn interpret(&self, interp: &mut I) -> Result<Cursor, Self::Error> {
        match self {
            Arith::Add {
                lhs, rhs, result, ..
            } => {
                let lhs = interp.read(*lhs)?;
                let rhs = interp.read(*rhs)?;
                interp.write(*result, lhs + rhs)?;
            }
            Arith::Sub {
                lhs, rhs, result, ..
            } => {
                let lhs = interp.read(*lhs)?;
                let rhs = interp.read(*rhs)?;
                interp.write(*result, lhs - rhs)?;
            }
            Arith::Mul {
                lhs, rhs, result, ..
            } => {
                let lhs = interp.read(*lhs)?;
                let rhs = interp.read(*rhs)?;
                interp.write(*result, lhs * rhs)?;
            }
            Arith::Div {
                lhs, rhs, result, ..
            } => {
                let lhs = interp.read(*lhs)?;
                let rhs = interp.read(*rhs)?;
                let value = lhs
                    .checked_div(rhs)
                    .ok_or_else(|| InterpreterError::custom(DivisionByZero))?;
                interp.write(*result, value)?;
            }
            Arith::Rem {
                lhs, rhs, result, ..
            } => {
                let lhs = interp.read(*lhs)?;
                let rhs = interp.read(*rhs)?;
                let value = lhs
                    .checked_rem(rhs)
                    .ok_or_else(|| InterpreterError::custom(DivisionByZero))?;
                interp.write(*result, value)?;
            }
            Arith::Neg {
                operand, result, ..
            } => {
                let operand = interp.read(*operand)?;
                interp.write(*result, -operand)?;
            }
            Self::__Phantom(..) => unreachable!(),
        }

        Ok(Cursor::Advance)
    }
}
