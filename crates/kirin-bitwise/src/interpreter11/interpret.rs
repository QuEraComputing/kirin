use std::ops::{BitAnd, BitOr, BitXor, Not};

use kirin::prelude::CompileTimeValue;
use kirin_interpreter_11::control::Control;
use kirin_interpreter_11::env::Env;
use kirin_interpreter_11::error::InterpreterError;
use kirin_interpreter_11::interpretable::Interpretable;

use crate::{Bitwise, CheckedShl, CheckedShr};

#[derive(Debug)]
struct ShiftOverflow;

impl std::fmt::Display for ShiftOverflow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "shift overflow")
    }
}

impl std::error::Error for ShiftOverflow {}

fn eval_impl<E, T>(op: &Bitwise<T>, env: &mut E) -> Result<(), E::Error>
where
    E: Env,
    E::Value: Clone
        + BitAnd<Output = E::Value>
        + BitOr<Output = E::Value>
        + BitXor<Output = E::Value>
        + Not<Output = E::Value>
        + CheckedShl
        + CheckedShr,
    E::Error: From<InterpreterError>,
    T: CompileTimeValue,
{
    match op {
        Bitwise::And {
            lhs, rhs, result, ..
        } => {
            let lhs = env.read(*lhs)?;
            let rhs = env.read(*rhs)?;
            env.write_result(*result, lhs & rhs)?;
        }
        Bitwise::Or {
            lhs, rhs, result, ..
        } => {
            let lhs = env.read(*lhs)?;
            let rhs = env.read(*rhs)?;
            env.write_result(*result, lhs | rhs)?;
        }
        Bitwise::Xor {
            lhs, rhs, result, ..
        } => {
            let lhs = env.read(*lhs)?;
            let rhs = env.read(*rhs)?;
            env.write_result(*result, lhs ^ rhs)?;
        }
        Bitwise::Not {
            operand, result, ..
        } => {
            let operand = env.read(*operand)?;
            env.write_result(*result, !operand)?;
        }
        Bitwise::Shl {
            lhs, rhs, result, ..
        } => {
            let lhs = env.read(*lhs)?;
            let rhs = env.read(*rhs)?;
            let value = lhs
                .checked_shl(rhs)
                .ok_or_else(|| E::Error::from(InterpreterError::Custom(Box::new(ShiftOverflow))))?;
            env.write_result(*result, value)?;
        }
        Bitwise::Shr {
            lhs, rhs, result, ..
        } => {
            let lhs = env.read(*lhs)?;
            let rhs = env.read(*rhs)?;
            let value = lhs
                .checked_shr(rhs)
                .ok_or_else(|| E::Error::from(InterpreterError::Custom(Box::new(ShiftOverflow))))?;
            env.write_result(*result, value)?;
        }
        Bitwise::__Phantom(..) => unreachable!(),
    }
    Ok(())
}

impl<E, T> Interpretable<E> for Bitwise<T>
where
    E: Env,
    E::Value: Clone
        + BitAnd<Output = E::Value>
        + BitOr<Output = E::Value>
        + BitXor<Output = E::Value>
        + Not<Output = E::Value>
        + CheckedShl
        + CheckedShr,
    E::Error: From<InterpreterError>,
    T: CompileTimeValue,
{
    fn eval(&self, env: &mut E) -> Result<Control<E::Value, E::Ext>, E::Error> {
        eval_impl(self, env)?;
        Ok(Control::Advance)
    }
}
