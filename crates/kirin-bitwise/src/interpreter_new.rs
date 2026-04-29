use std::ops::{BitAnd, BitOr, BitXor, Not};

use kirin::prelude::{CompileTimeValue, Dialect, SSAValue};
use kirin_interpreter_new::{
    BlockTransfer, Env, Interpretable, InterpreterError, Location, StatementEffect,
};

use crate::{Bitwise, CheckedShl, CheckedShr};

impl<L, I, F, C, E, V, T> Interpretable<L, I, F, C, E, BlockTransfer<V>> for Bitwise<T>
where
    L: Dialect,
    I: Env<V, Error = E>,
    V: Clone
        + BitAnd<Output = V>
        + BitOr<Output = V>
        + BitXor<Output = V>
        + Not<Output = V>
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
    ) -> Result<StatementEffect<F, C, BlockTransfer<V>>, E> {
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
                    .ok_or(ShiftOverflow)?;
                interp.write(env, SSAValue::from(*result), value)?;
            }
            Bitwise::Shr {
                lhs, rhs, result, ..
            } => {
                let value = interp
                    .read(env, *lhs)?
                    .checked_shr(interp.read(env, *rhs)?)
                    .ok_or(ShiftOverflow)?;
                interp.write(env, SSAValue::from(*result), value)?;
            }
            Bitwise::__Phantom(..) => unreachable!(),
        }
        Ok(StatementEffect::Done)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShiftOverflow;

impl std::fmt::Display for ShiftOverflow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "shift overflow")
    }
}

impl std::error::Error for ShiftOverflow {}

impl From<ShiftOverflow> for InterpreterError {
    fn from(_: ShiftOverflow) -> Self {
        Self::Custom("shift overflow")
    }
}
