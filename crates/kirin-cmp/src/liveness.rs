//! Future liveness semantics for comparison statements.
//!
//! This file is intentionally not wired into `lib.rs` yet. It sketches where a
//! future backward/liveness interpretation of `Cmp<T>` should live once the
//! liveness context and effect algebra exist.

/*
use kirin::prelude::CompileTimeValue;
use kirin_interpreter::dialect::Interpretable;
use kirin_liveness::{LivenessContext, LivenessInterp};

use crate::{Cmp, CompareValue};

impl<I, T> Interpretable<LivenessContext<'_, I>> for Cmp<T>
where
    I: LivenessInterp,
    T: CompileTimeValue,
{
    fn interpret(
        &self,
        ctx: &mut LivenessContext<'_, I>,
    ) -> Result<I::Effect, I::Error> {
        match self {
            Cmp::Eq { lhs, rhs, result, .. }
            | Cmp::Ne { lhs, rhs, result, .. }
            | Cmp::Lt { lhs, rhs, result, .. }
            | Cmp::Le { lhs, rhs, result, .. }
            | Cmp::Gt { lhs, rhs, result, .. }
            | Cmp::Ge { lhs, rhs, result, .. } => {
                // Classic backward liveness shape:
                // live_before = uses(stmt) union (live_after - defs(stmt)).
                ctx.use_def([*lhs, *rhs], [*result])
            }
            Cmp::__Phantom(..) => unreachable!(),
        }
    }
}
*/
