use std::ops::{BitAnd, BitOr, BitXor, Not};

use kirin::prelude::CompileTimeValue;
use kirin_interpreter_6::env::{Env, Interpretable};
use kirin_interpreter_6::error::InterpreterError;
use kirin_interpreter_6::lift::Lift;

use crate::{Bitwise, CheckedShl, CheckedShr};

#[derive(Debug)]
struct ShiftOverflow;

impl std::fmt::Display for ShiftOverflow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "shift overflow")
    }
}

impl std::error::Error for ShiftOverflow {}

impl<E, T> Interpretable<E> for Bitwise<T>
where
    E: Env,
    E::Effect: Lift<()>,
    E::Value: Clone
        + BitAnd<Output = E::Value>
        + BitOr<Output = E::Value>
        + BitXor<Output = E::Value>
        + Not<Output = E::Value>
        + CheckedShl
        + CheckedShr,
    T: CompileTimeValue,
{
    type DialectEffect = E::Effect;

    fn interpret(&self, env: &mut E) -> Result<E::Effect, E::Error> {
        match self {
            Bitwise::And {
                lhs, rhs, result, ..
            } => {
                let lhs = env.read(*lhs)?;
                let rhs = env.read(*rhs)?;
                env.write(*result, lhs & rhs)?;
            }
            Bitwise::Or {
                lhs, rhs, result, ..
            } => {
                let lhs = env.read(*lhs)?;
                let rhs = env.read(*rhs)?;
                env.write(*result, lhs | rhs)?;
            }
            Bitwise::Xor {
                lhs, rhs, result, ..
            } => {
                let lhs = env.read(*lhs)?;
                let rhs = env.read(*rhs)?;
                env.write(*result, lhs ^ rhs)?;
            }
            Bitwise::Not {
                operand, result, ..
            } => {
                let operand = env.read(*operand)?;
                env.write(*result, !operand)?;
            }
            Bitwise::Shl {
                lhs, rhs, result, ..
            } => {
                let lhs = env.read(*lhs)?;
                let rhs = env.read(*rhs)?;
                let value = lhs.checked_shl(rhs).ok_or_else(|| {
                    E::Error::from(InterpreterError::Custom(Box::new(ShiftOverflow)))
                })?;
                env.write(*result, value)?;
            }
            Bitwise::Shr {
                lhs, rhs, result, ..
            } => {
                let lhs = env.read(*lhs)?;
                let rhs = env.read(*rhs)?;
                let value = lhs.checked_shr(rhs).ok_or_else(|| {
                    E::Error::from(InterpreterError::Custom(Box::new(ShiftOverflow)))
                })?;
                env.write(*result, value)?;
            }
            Self::__Phantom(..) => unreachable!(),
        }
        Ok(E::advance())
    }
}
