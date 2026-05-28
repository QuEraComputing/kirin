use std::ops::{Add, Mul, Neg, Sub};

use kirin::prelude::{CompileTimeValue, Dialect, SSAValue};
use kirin_interpreter::{
    BlockTransfer, Env, Interpretable, InterpreterError, Location, StatementEffect,
};
use thiserror::Error;

use crate::{Arith, CheckedDiv, CheckedRem};

impl<L, I, F, C, E, T, X> Interpretable<L, I, F, C, E, X> for Arith<T>
where
    L: Dialect,
    I: Env<X::Value, Error = E>,
    X: BlockTransfer,
    X::Value: Clone
        + Add<Output = X::Value>
        + Sub<Output = X::Value>
        + Mul<Output = X::Value>
        + Neg<Output = X::Value>
        + CheckedDiv
        + CheckedRem,
    E: From<DivisionByZero>,
    T: CompileTimeValue,
{
    fn interpret(
        &self,
        _location: Location,
        env: kirin_interpreter::EnvIndex,
        interp: &mut I,
    ) -> Result<StatementEffect<F, C, X>, E> {
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
                    .ok_or_else(|| E::from(DivisionByZero))?;
                interp.write(env, SSAValue::from(*result), value)?;
            }
            Arith::Rem {
                lhs, rhs, result, ..
            } => {
                let value = interp
                    .read(env, *lhs)?
                    .checked_rem(interp.read(env, *rhs)?)
                    .ok_or_else(|| E::from(DivisionByZero))?;
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

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
#[error("division by zero")]
pub struct DivisionByZero;

impl From<DivisionByZero> for InterpreterError {
    fn from(_: DivisionByZero) -> Self {
        Self::Custom("division by zero")
    }
}
