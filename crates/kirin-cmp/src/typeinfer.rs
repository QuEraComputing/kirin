//! Future type-inference semantics for comparison statements.
//!
//! This file is intentionally not wired into `lib.rs` yet. It sketches where a
//! future type interpretation of `Cmp<T>` should live once the type context and
//! effect algebra exist.

/*
use kirin::prelude::CompileTimeValue;
use kirin_interpreter::dialect::Interpretable;
use kirin_typeinfer::{TypeContext, TypeInterp, TypeValue};

use crate::{Cmp, CompareValue};

impl<I, T> Interpretable<TypeContext<'_, I>> for Cmp<T>
where
    I: TypeInterp,
    T: CompileTimeValue,
{
    fn interpret(&self, ctx: &mut TypeContext<'_, I>) -> Result<I::Effect, I::Error> {
        match self {
            Cmp::Eq { lhs, rhs, result, .. }
            | Cmp::Ne { lhs, rhs, result, .. }
            | Cmp::Lt { lhs, rhs, result, .. }
            | Cmp::Le { lhs, rhs, result, .. }
            | Cmp::Gt { lhs, rhs, result, .. }
            | Cmp::Ge { lhs, rhs, result, .. } => {
                let lhs_ty = ctx.type_of(*lhs)?;
                let rhs_ty = ctx.type_of(*rhs)?;
                ctx.require_comparable(lhs_ty, rhs_ty)?;
                ctx.set_type(*result, TypeValue::Bool)?;
                Ok(I::Effect::default_next())
            }
            Cmp::__Phantom(..) => unreachable!(),
        }
    }
}
*/
