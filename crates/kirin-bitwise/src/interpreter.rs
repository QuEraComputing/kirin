use std::ops::{BitAnd, BitOr, BitXor, Not};

use kirin::prelude::CompileTimeValue;
use kirin_interpreter::dialect::{
    ForwardContext, ForwardCtx, ForwardEffect, ForwardInterp, Interpretable, InterpreterError,
};
use thiserror::Error;

use crate::{Bitwise, CheckedShl, CheckedShr};

impl<I, T> Interpretable<ForwardContext<'_, I>> for Bitwise<T>
where
    I: ForwardInterp,
    I::Value: BitAnd<Output = I::Value>
        + BitOr<Output = I::Value>
        + BitXor<Output = I::Value>
        + Not<Output = I::Value>
        + CheckedShl
        + CheckedShr,
    I::Error: From<ShiftOverflow>,
    T: CompileTimeValue,
{
    fn interpret(&self, ctx: &mut ForwardContext<'_, I>) -> Result<I::Effect, I::Error> {
        match self {
            Bitwise::And {
                lhs, rhs, result, ..
            } => {
                let value = ctx.read(*lhs)? & ctx.read(*rhs)?;
                ctx.write(*result, value)?;
            }
            Bitwise::Or {
                lhs, rhs, result, ..
            } => {
                let value = ctx.read(*lhs)? | ctx.read(*rhs)?;
                ctx.write(*result, value)?;
            }
            Bitwise::Xor {
                lhs, rhs, result, ..
            } => {
                let value = ctx.read(*lhs)? ^ ctx.read(*rhs)?;
                ctx.write(*result, value)?;
            }
            Bitwise::Not {
                operand, result, ..
            } => {
                let value = !ctx.read(*operand)?;
                ctx.write(*result, value)?;
            }
            Bitwise::Shl {
                lhs, rhs, result, ..
            } => {
                let value = ctx
                    .read(*lhs)?
                    .checked_shl(ctx.read(*rhs)?)
                    .ok_or_else(|| I::Error::from(ShiftOverflow))?;
                ctx.write(*result, value)?;
            }
            Bitwise::Shr {
                lhs, rhs, result, ..
            } => {
                let value = ctx
                    .read(*lhs)?
                    .checked_shr(ctx.read(*rhs)?)
                    .ok_or_else(|| I::Error::from(ShiftOverflow))?;
                ctx.write(*result, value)?;
            }
            Bitwise::__Phantom(..) => unreachable!(),
        }
        Ok(ForwardEffect::Next)
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
