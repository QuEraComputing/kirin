use std::ops::{BitAnd, BitOr, BitXor, Not};

use kirin::prelude::{CompileTimeValue, Dialect, SSAValue};
use kirin_interpreter_new::{
    BlockTransfer, Env, Interpretable, InterpreterError, Location, StatementEffect,
};
use thiserror::Error;

use crate::{Bitwise, CheckedShl, CheckedShr};

impl<L, I, F, C, E, T, X> Interpretable<L, I, F, C, E, X> for Bitwise<T>
where
    L: Dialect,
    I: Env<X::Value, Error = E>,
    X: BlockTransfer,
    X::Value: Clone
        + BitAnd<Output = X::Value>
        + BitOr<Output = X::Value>
        + BitXor<Output = X::Value>
        + Not<Output = X::Value>
        + CheckedShl
        + CheckedShr,
    E: From<ShiftOverflow>,
    T: CompileTimeValue,
{
    fn interpret(
        &self,
        _location: Location,
        env: kirin_interpreter_new::EnvIndex,
        interp: &mut I,
    ) -> Result<StatementEffect<F, C, X>, E> {
        match self {
            Bitwise::And {
                lhs, rhs, result, ..
            } => {
                let value = interp.read(env, *lhs)? & interp.read(env, *rhs)?;
                interp.write(env, SSAValue::from(*result), value)?;
            }
            Bitwise::Or {
                lhs, rhs, result, ..
            } => {
                let value = interp.read(env, *lhs)? | interp.read(env, *rhs)?;
                interp.write(env, SSAValue::from(*result), value)?;
            }
            Bitwise::Xor {
                lhs, rhs, result, ..
            } => {
                let value = interp.read(env, *lhs)? ^ interp.read(env, *rhs)?;
                interp.write(env, SSAValue::from(*result), value)?;
            }
            Bitwise::Not {
                operand, result, ..
            } => {
                let value = !interp.read(env, *operand)?;
                interp.write(env, SSAValue::from(*result), value)?;
            }
            Bitwise::Shl {
                lhs, rhs, result, ..
            } => {
                let value = interp
                    .read(env, *lhs)?
                    .checked_shl(interp.read(env, *rhs)?)
                    .ok_or_else(|| E::from(ShiftOverflow))?;
                interp.write(env, SSAValue::from(*result), value)?;
            }
            Bitwise::Shr {
                lhs, rhs, result, ..
            } => {
                let value = interp
                    .read(env, *lhs)?
                    .checked_shr(interp.read(env, *rhs)?)
                    .ok_or_else(|| E::from(ShiftOverflow))?;
                interp.write(env, SSAValue::from(*result), value)?;
            }
            Bitwise::__Phantom(..) => unreachable!(),
        }
        Ok(StatementEffect::Done)
    }
}

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
#[error("shift overflow")]
pub struct ShiftOverflow;

impl From<ShiftOverflow> for InterpreterError {
    fn from(_: ShiftOverflow) -> Self {
        Self::Custom("shift overflow")
    }
}
