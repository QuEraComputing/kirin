use std::ops::{BitAnd, BitOr, BitXor, Not, Shl, Shr};

use kirin::prelude::{CompileTimeValue, Dialect};
use kirin_interpreter::{Continuation, Interpretable, Interpreter, InterpreterError};

use crate::Bitwise;

impl<'ir, I, L, T> Interpretable<'ir, I, L> for Bitwise<T>
where
    I: Interpreter<'ir>,
    I::Value: Clone
        + BitAnd<Output = I::Value>
        + BitOr<Output = I::Value>
        + BitXor<Output = I::Value>
        + Not<Output = I::Value>
        + Shl<Output = I::Value>
        + Shr<Output = I::Value>,
    I::Error: From<InterpreterError>,
    L: Dialect,
    T: CompileTimeValue + Default,
{
    fn interpret(&self, interp: &mut I) -> Result<Continuation<I::Value, I::Ext>, I::Error> {
        match self {
            Bitwise::And {
                lhs, rhs, result, ..
            } => {
                let a = interp.read(*lhs)?;
                let b = interp.read(*rhs)?;
                interp.write(*result, a & b)?;
                Ok(Continuation::Continue)
            }
            Bitwise::Or {
                lhs, rhs, result, ..
            } => {
                let a = interp.read(*lhs)?;
                let b = interp.read(*rhs)?;
                interp.write(*result, a | b)?;
                Ok(Continuation::Continue)
            }
            Bitwise::Xor {
                lhs, rhs, result, ..
            } => {
                let a = interp.read(*lhs)?;
                let b = interp.read(*rhs)?;
                interp.write(*result, a ^ b)?;
                Ok(Continuation::Continue)
            }
            Bitwise::Not {
                operand, result, ..
            } => {
                let a = interp.read(*operand)?;
                interp.write(*result, !a)?;
                Ok(Continuation::Continue)
            }
            Bitwise::Shl {
                lhs, rhs, result, ..
            } => {
                let a = interp.read(*lhs)?;
                let b = interp.read(*rhs)?;
                interp.write(*result, a << b)?;
                Ok(Continuation::Continue)
            }
            Bitwise::Shr {
                lhs, rhs, result, ..
            } => {
                let a = interp.read(*lhs)?;
                let b = interp.read(*rhs)?;
                interp.write(*result, a >> b)?;
                Ok(Continuation::Continue)
            }
        }
    }
}
