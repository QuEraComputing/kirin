use std::ops::{Add, Mul, Neg, Sub};

use kirin::prelude::CompileTimeValue;
use kirin_interpreter_4::effect::CursorEffect;
use kirin_interpreter_4::error::InterpreterError;
use kirin_interpreter_4::lift::LiftInto;
use kirin_interpreter_4::traits::{Interpretable, Interpreter, Machine, ValueStore};

use crate::{Arith, CheckedDiv, CheckedRem};

#[derive(Debug)]
struct DivisionByZero;

impl std::fmt::Display for DivisionByZero {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "division by zero")
    }
}

impl std::error::Error for DivisionByZero {}

impl<I, T> Interpretable<I> for Arith<T>
where
    I: Interpreter + Machine<Error = InterpreterError>,
    <I as ValueStore>::Value: Clone
        + Add<Output = <I as ValueStore>::Value>
        + Sub<Output = <I as ValueStore>::Value>
        + Mul<Output = <I as ValueStore>::Value>
        + CheckedDiv
        + CheckedRem
        + Neg<Output = <I as ValueStore>::Value>,
    CursorEffect<<I as ValueStore>::Value>: LiftInto<<I as Machine>::Effect>,
    T: CompileTimeValue,
{
    type Effect = CursorEffect<<I as ValueStore>::Value>;
    type Error = InterpreterError;

    fn interpret(
        &self,
        interp: &mut I,
    ) -> Result<CursorEffect<<I as ValueStore>::Value>, InterpreterError> {
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
                    .ok_or_else(|| InterpreterError::Custom(Box::new(DivisionByZero)))?;
                interp.write(*result, value)?;
            }
            Arith::Rem {
                lhs, rhs, result, ..
            } => {
                let lhs = interp.read(*lhs)?;
                let rhs = interp.read(*rhs)?;
                let value = lhs
                    .checked_rem(rhs)
                    .ok_or_else(|| InterpreterError::Custom(Box::new(DivisionByZero)))?;
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

        Ok(CursorEffect::Advance)
    }
}
