use std::ops::{Add, Mul, Neg, Sub};

use kirin::prelude::{CompileTimeValue, Dialect, SSAValue};
use kirin_interpreter_new::{
    BlockTransfer, Env, Interpretable, InterpreterError, Location, StatementEffect,
};

use crate::{Arith, CheckedDiv, CheckedRem};

impl<L, I, F, C, E, V, T> Interpretable<L, I, F, C, E, BlockTransfer<V>> for Arith<T>
where
    L: Dialect,
    I: Env<V, Error = E>,
    V: Clone
        + Add<Output = V>
        + Sub<Output = V>
        + Mul<Output = V>
        + Neg<Output = V>
        + CheckedDiv
        + CheckedRem,
    E: From<DivisionByZero>,
    T: CompileTimeValue,
{
    fn interpret(
        &self,
        _location: Location,
        env: kirin_interpreter_new::EnvIndex,
        interp: &mut I,
    ) -> Result<StatementEffect<F, C, BlockTransfer<V>>, E> {
        match self {
            Arith::Add {
                lhs, rhs, result, ..
            } => {
                interp.write(
                    env,
                    SSAValue::from(*result),
                    interp.read(env, *lhs)? + interp.read(env, *rhs)?,
                )?;
            }
            Arith::Sub {
                lhs, rhs, result, ..
            } => {
                interp.write(
                    env,
                    SSAValue::from(*result),
                    interp.read(env, *lhs)? - interp.read(env, *rhs)?,
                )?;
            }
            Arith::Mul {
                lhs, rhs, result, ..
            } => {
                interp.write(
                    env,
                    SSAValue::from(*result),
                    interp.read(env, *lhs)? * interp.read(env, *rhs)?,
                )?;
            }
            Arith::Div {
                lhs, rhs, result, ..
            } => {
                let value = interp
                    .read(env, *lhs)?
                    .checked_div(interp.read(env, *rhs)?)
                    .ok_or(DivisionByZero)?;
                interp.write(env, SSAValue::from(*result), value)?;
            }
            Arith::Rem {
                lhs, rhs, result, ..
            } => {
                let value = interp
                    .read(env, *lhs)?
                    .checked_rem(interp.read(env, *rhs)?)
                    .ok_or(DivisionByZero)?;
                interp.write(env, SSAValue::from(*result), value)?;
            }
            Arith::Neg {
                operand, result, ..
            } => {
                let value = -interp.read(env, *operand)?;
                interp.write(env, SSAValue::from(*result), value)?;
            }
            Arith::__Phantom(..) => unreachable!(),
        }
        Ok(StatementEffect::Done)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DivisionByZero;

impl std::fmt::Display for DivisionByZero {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "division by zero")
    }
}

impl std::error::Error for DivisionByZero {}

impl From<DivisionByZero> for InterpreterError {
    fn from(_: DivisionByZero) -> Self {
        Self::Custom("division by zero")
    }
}
