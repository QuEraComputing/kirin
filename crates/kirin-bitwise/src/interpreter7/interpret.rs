use std::ops::{BitAnd, BitOr, BitXor, Not};

use kirin::prelude::{CompileTimeValue, Dialect, HasStageInfo, StageMeta};
use kirin_interpreter::AbstractValue;
use kirin_interpreter_7::abstract_interp::AbstractInterp;
use kirin_interpreter_7::concrete::ConcreteInterp;
use kirin_interpreter_7::env::Interpretable;
use kirin_interpreter_7::error::InterpreterError;
use kirin_interpreter_7::store::Store;

use crate::{Bitwise, CheckedShl, CheckedShr};

#[derive(Debug)]
struct ShiftOverflow;

impl std::fmt::Display for ShiftOverflow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "shift overflow")
    }
}

impl std::error::Error for ShiftOverflow {}

fn interp_impl<S, T>(op: &Bitwise<T>, env: &mut S) -> Result<(), S::Error>
where
    S: Store,
    S::Value: Clone
        + BitAnd<Output = S::Value>
        + BitOr<Output = S::Value>
        + BitXor<Output = S::Value>
        + Not<Output = S::Value>
        + CheckedShl
        + CheckedShr,
    S::Error: From<InterpreterError>,
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
                .ok_or_else(|| S::Error::from(InterpreterError::Custom(Box::new(ShiftOverflow))))?;
            env.write_result(*result, value)?;
        }
        Bitwise::Shr {
            lhs, rhs, result, ..
        } => {
            let lhs = env.read(*lhs)?;
            let rhs = env.read(*rhs)?;
            let value = lhs
                .checked_shr(rhs)
                .ok_or_else(|| S::Error::from(InterpreterError::Custom(Box::new(ShiftOverflow))))?;
            env.write_result(*result, value)?;
        }
        Bitwise::__Phantom(..) => unreachable!(),
    }
    Ok(())
}

impl<'ir, S, L, V, C, T> Interpretable<ConcreteInterp<'ir, S, L, V, C>> for Bitwise<T>
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
    V: Clone
        + BitAnd<Output = V>
        + BitOr<Output = V>
        + BitXor<Output = V>
        + Not<Output = V>
        + CheckedShl
        + CheckedShr,
    C: 'static,
    T: CompileTimeValue,
{
    type Effect = ();

    fn interpret(&self, env: &mut ConcreteInterp<'ir, S, L, V, C>) -> Result<(), InterpreterError> {
        interp_impl(self, env)
    }
}

impl<'ir, S, L, V, T> Interpretable<AbstractInterp<'ir, S, L, V>> for Bitwise<T>
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
    V: Clone
        + AbstractValue
        + BitAnd<Output = V>
        + BitOr<Output = V>
        + BitXor<Output = V>
        + Not<Output = V>
        + CheckedShl
        + CheckedShr,
    T: CompileTimeValue,
{
    type Effect = ();

    fn interpret(&self, env: &mut AbstractInterp<'ir, S, L, V>) -> Result<(), InterpreterError> {
        interp_impl(self, env)
    }
}
