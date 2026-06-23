use std::ops::{Add, Mul, Neg, Sub};

use kirin::prelude::CompileTimeValue;
use kirin_interpreter::dialect::{
    ForwardContext, ForwardCtx, ForwardEffect, ForwardInterp, Interpretable, InterpreterError,
};
use thiserror::Error;

use crate::{Arith, CheckedDiv, CheckedRem};

impl<I, T> Interpretable<ForwardContext<'_, I>> for Arith<T>
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
    fn interpret(&self, ctx: &mut ForwardContext<'_, I>) -> Result<I::Effect, I::Error> {
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

/// Coherence regression test: a *future* backward-liveness analysis must be able
/// to add `Interpretable<LivenessContext<'_, I>>` impls for the same dialect
/// statements as the forward `Interpretable<ForwardContext<'_, I>>` impl above,
/// **without** `E0119` overlap — even though both are generic over the engine `I`.
///
/// This works only because `Interpretable` is specialized on the *context type*:
/// `ForwardContext` and `LivenessContext` are different type constructors, so the trait
/// solver sees the two impls as disjoint. (Two impls keyed on `I` alone, differing
/// only in a `where I: ForwardInterp` vs `where I: LiveInterp` bound, would overlap
/// — coherence ignores those bounds.) No liveness analysis is implemented here;
/// this is purely a compile-time proof the framework seam is ready.
#[cfg(test)]
mod liveness_context_disjoint {
    use std::marker::PhantomData;

    use kirin::prelude::CompileTimeValue;
    use kirin_interpreter::dialect::{InterpretCtx, Interpretable, InterpreterError};

    use crate::Arith;

    /// A stand-in for a future liveness context: a *distinct* concrete context
    /// type exposing its own effect (here a trivial live-set) — deliberately not a
    /// `ForwardCtx`, so it never offers forward read/write.
    struct LivenessContext<'a, I>(PhantomData<&'a mut I>);

    impl<'a, I> InterpretCtx for LivenessContext<'a, I> {
        type Value = ();
        type Error = InterpreterError;
        type Effect = ();
    }

    // The mock future-liveness rule for `Arith`, specialized on `LivenessContext`.
    // Its coexistence with the forward `Interpretable<ForwardContext<'_, I>>` impl
    // above is what this test asserts at compile time.
    impl<I, T> Interpretable<LivenessContext<'_, I>> for Arith<T>
    where
        T: CompileTimeValue,
    {
        fn interpret(&self, _ctx: &mut LivenessContext<'_, I>) -> Result<(), InterpreterError> {
            Ok(())
        }
    }

    #[test]
    fn forward_and_liveness_context_impls_coexist() {
        // If this module compiles, the two context-specialized `Interpretable`
        // impls for `Arith` do not overlap. Nothing to assert at runtime.
    }
}
