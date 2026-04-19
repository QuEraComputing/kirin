use std::ops::{BitAnd, BitOr, BitXor, Not};

use kirin::prelude::CompileTimeValue;
use kirin_interpreter_13::control::Control;
use kirin_interpreter_13::env::Env;
use kirin_interpreter_13::error::InterpreterError;
use kirin_interpreter_13::interpretable::Interpretable;

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
            env.write_result(*result, env.read(*lhs)? & env.read(*rhs)?)?;
        }
        Bitwise::Or {
            lhs, rhs, result, ..
        } => {
            env.write_result(*result, env.read(*lhs)? | env.read(*rhs)?)?;
        }
        Bitwise::Xor {
            lhs, rhs, result, ..
        } => {
            env.write_result(*result, env.read(*lhs)? ^ env.read(*rhs)?)?;
        }
        Bitwise::Not {
            operand, result, ..
        } => {
            env.write_result(*result, !env.read(*operand)?)?;
        }
        Bitwise::Shl {
            lhs, rhs, result, ..
        } => {
            let v = env
                .read(*lhs)?
                .checked_shl(env.read(*rhs)?)
                .ok_or_else(|| E::Error::from(InterpreterError::Custom(Box::new(ShiftOverflow))))?;
            env.write_result(*result, v)?;
        }
        Bitwise::Shr {
            lhs, rhs, result, ..
        } => {
            let v = env
                .read(*lhs)?
                .checked_shr(env.read(*rhs)?)
                .ok_or_else(|| E::Error::from(InterpreterError::Custom(Box::new(ShiftOverflow))))?;
            env.write_result(*result, v)?;
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
