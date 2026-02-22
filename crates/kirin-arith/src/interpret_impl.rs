use std::ops::{Add, Div, Mul, Neg, Rem, Sub};

use kirin::prelude::{CompileTimeValue, Dialect};
use kirin_interpreter::{Continuation, Interpretable, Interpreter, InterpreterError};

use crate::Arith;

impl<I, L, T> Interpretable<I, L> for Arith<T>
where
    I: Interpreter,
    I::Value: Clone
        + Add<Output = I::Value>
        + Sub<Output = I::Value>
        + Mul<Output = I::Value>
        + Div<Output = I::Value>
        + Rem<Output = I::Value>
        + Neg<Output = I::Value>,
    I::Error: From<InterpreterError>,
    L: Dialect,
    T: CompileTimeValue + Default,
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
                interp.write(*result, a / b)?;
                Ok(Continuation::Continue)
            }
            Arith::Rem {
                lhs, rhs, result, ..
            } => {
                let a = interp.read(*lhs)?;
                let b = interp.read(*rhs)?;
                interp.write(*result, a % b)?;
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
