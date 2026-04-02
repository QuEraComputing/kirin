use std::ops::{BitAnd, BitOr, BitXor, Not};

use kirin::prelude::CompileTimeValue;
use kirin_interpreter_4::effect::CursorEffect;
use kirin_interpreter_4::error::InterpreterError;
use kirin_interpreter_4::lift::LiftInto;
use kirin_interpreter_4::traits::{Interpretable, Interpreter, Machine, ValueStore};

use crate::{Bitwise, CheckedShl, CheckedShr};

#[derive(Debug)]
struct ShiftOverflow;

impl std::fmt::Display for ShiftOverflow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "shift overflow")
    }
}

impl std::error::Error for ShiftOverflow {}

impl<I, T> Interpretable<I> for Bitwise<T>
where
    I: Interpreter + Machine<Error = InterpreterError>,
    <I as ValueStore>::Value: Clone
        + BitAnd<Output = <I as ValueStore>::Value>
        + BitOr<Output = <I as ValueStore>::Value>
        + BitXor<Output = <I as ValueStore>::Value>
        + Not<Output = <I as ValueStore>::Value>
        + CheckedShl
        + CheckedShr,
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
            Bitwise::And {
                lhs, rhs, result, ..
            } => {
                let lhs = interp.read(*lhs)?;
                let rhs = interp.read(*rhs)?;
                interp.write(*result, lhs & rhs)?;
            }
            Bitwise::Or {
                lhs, rhs, result, ..
            } => {
                let lhs = interp.read(*lhs)?;
                let rhs = interp.read(*rhs)?;
                interp.write(*result, lhs | rhs)?;
            }
            Bitwise::Xor {
                lhs, rhs, result, ..
            } => {
                let lhs = interp.read(*lhs)?;
                let rhs = interp.read(*rhs)?;
                interp.write(*result, lhs ^ rhs)?;
            }
            Bitwise::Not {
                operand, result, ..
            } => {
                let operand = interp.read(*operand)?;
                interp.write(*result, !operand)?;
            }
            Bitwise::Shl {
                lhs, rhs, result, ..
            } => {
                let lhs = interp.read(*lhs)?;
                let rhs = interp.read(*rhs)?;
                let value = lhs
                    .checked_shl(rhs)
                    .ok_or_else(|| InterpreterError::Custom(Box::new(ShiftOverflow)))?;
                interp.write(*result, value)?;
            }
            Bitwise::Shr {
                lhs, rhs, result, ..
            } => {
                let lhs = interp.read(*lhs)?;
                let rhs = interp.read(*rhs)?;
                let value = lhs
                    .checked_shr(rhs)
                    .ok_or_else(|| InterpreterError::Custom(Box::new(ShiftOverflow)))?;
                interp.write(*result, value)?;
            }
            Self::__Phantom(..) => unreachable!(),
        }

        Ok(CursorEffect::Advance)
    }
}
