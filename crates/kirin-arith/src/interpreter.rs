use std::ops::{Add, Mul, Neg, Sub};

use kirin::prelude::CompileTimeValue;
use kirin_interpreter::dialect::{
    Ctx, ForwardEffect, ForwardInterp, Interpretable, InterpreterError,
};
use thiserror::Error;

use crate::{Arith, CheckedDiv, CheckedRem};

impl<I, T> Interpretable<I> for Arith<T>
where
    I: ForwardInterp,
    I::Value: Add<Output = I::Value>
        + Sub<Output = I::Value>
        + Mul<Output = I::Value>
        + Neg<Output = I::Value>
        + CheckedDiv
        + CheckedRem,
    I::Error: From<DivisionByZero>,
    T: CompileTimeValue,
{
    fn interpret(&self, ctx: &mut Ctx<'_, I>) -> Result<I::Effect, I::Error> {
        match self {
            Arith::Add {
                lhs, rhs, result, ..
            } => {
                let value = ctx.read(*lhs)? + ctx.read(*rhs)?;
                ctx.write(*result, value)?;
            }
            Arith::Sub {
                lhs, rhs, result, ..
            } => {
                let value = ctx.read(*lhs)? - ctx.read(*rhs)?;
                ctx.write(*result, value)?;
            }
            Arith::Mul {
                lhs, rhs, result, ..
            } => {
                let value = ctx.read(*lhs)? * ctx.read(*rhs)?;
                ctx.write(*result, value)?;
            }
            Arith::Div {
                lhs, rhs, result, ..
            } => {
                let value = ctx
                    .read(*lhs)?
                    .checked_div(ctx.read(*rhs)?)
                    .ok_or_else(|| I::Error::from(DivisionByZero))?;
                ctx.write(*result, value)?;
            }
            Arith::Rem {
                lhs, rhs, result, ..
            } => {
                let value = ctx
                    .read(*lhs)?
                    .checked_rem(ctx.read(*rhs)?)
                    .ok_or_else(|| I::Error::from(DivisionByZero))?;
                ctx.write(*result, value)?;
            }
            Arith::Neg {
                operand, result, ..
            } => {
                let value = -ctx.read(*operand)?;
                ctx.write(*result, value)?;
            }
            Arith::__Phantom(..) => unreachable!(),
        }
        Ok(ForwardEffect::Next)
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
