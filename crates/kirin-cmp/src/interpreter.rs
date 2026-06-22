use kirin::prelude::CompileTimeValue;
use kirin_interpreter::dialect::{Ctx, ForwardEffect, ForwardInterp, Interpretable};

use crate::{Cmp, CompareValue};

impl<I, T> Interpretable<I> for Cmp<T>
where
    I: ForwardInterp,
    I::Value: CompareValue,
    <I::Value as CompareValue>::Bool: Into<I::Value>,
    T: CompileTimeValue,
{
    fn interpret(&self, ctx: &mut Ctx<'_, I>) -> Result<I::Effect, I::Error> {
        match self {
            Cmp::Eq {
                lhs, rhs, result, ..
            } => {
                let value = ctx.read(*lhs)?.cmp_eq(&ctx.read(*rhs)?).into();
                ctx.write(*result, value)?;
            }
            Cmp::Ne {
                lhs, rhs, result, ..
            } => {
                let value = ctx.read(*lhs)?.cmp_ne(&ctx.read(*rhs)?).into();
                ctx.write(*result, value)?;
            }
            Cmp::Lt {
                lhs, rhs, result, ..
            } => {
                let value = ctx.read(*lhs)?.cmp_lt(&ctx.read(*rhs)?).into();
                ctx.write(*result, value)?;
            }
            Cmp::Le {
                lhs, rhs, result, ..
            } => {
                let value = ctx.read(*lhs)?.cmp_le(&ctx.read(*rhs)?).into();
                ctx.write(*result, value)?;
            }
            Cmp::Gt {
                lhs, rhs, result, ..
            } => {
                let value = ctx.read(*lhs)?.cmp_gt(&ctx.read(*rhs)?).into();
                ctx.write(*result, value)?;
            }
            Cmp::Ge {
                lhs, rhs, result, ..
            } => {
                let value = ctx.read(*lhs)?.cmp_ge(&ctx.read(*rhs)?).into();
                ctx.write(*result, value)?;
            }
            Cmp::__Phantom(..) => unreachable!(),
        }
        Ok(ForwardEffect::Next)
    }
}
