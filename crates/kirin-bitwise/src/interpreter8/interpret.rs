use std::ops::{BitAnd, BitOr, BitXor, Not};

use kirin::prelude::{CompileTimeValue, Dialect, HasStageInfo, StageMeta};
use kirin_interpreter::AbstractValue;
use kirin_interpreter_8::abstract_interp::AbstractInterp;
use kirin_interpreter_8::concrete::ConcreteInterp;
use kirin_interpreter_8::env::Env;
use kirin_interpreter_8::error::InterpreterError;
use kirin_interpreter_8::semantics::Semantics;

use crate::{Bitwise, CheckedShl, CheckedShr};

#[derive(Debug)]
struct ShiftOverflow;

impl std::fmt::Display for ShiftOverflow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "shift overflow")
    }
}

impl std::error::Error for ShiftOverflow {}

fn eval_impl<D, T>(op: &Bitwise<T>, domain: &mut D) -> Result<(), D::Error>
where
    D: Env,
    D::Value: Clone
        + BitAnd<Output = D::Value>
        + BitOr<Output = D::Value>
        + BitXor<Output = D::Value>
        + Not<Output = D::Value>
        + CheckedShl
        + CheckedShr,
    D::Error: From<InterpreterError>,
    T: CompileTimeValue,
{
    match op {
        Bitwise::And {
            lhs, rhs, result, ..
        } => {
            let lhs = domain.read_value(*lhs)?;
            let rhs = domain.read_value(*rhs)?;
            domain.write_result(*result, lhs & rhs)?;
        }
        Bitwise::Or {
            lhs, rhs, result, ..
        } => {
            let lhs = domain.read_value(*lhs)?;
            let rhs = domain.read_value(*rhs)?;
            domain.write_result(*result, lhs | rhs)?;
        }
        Bitwise::Xor {
            lhs, rhs, result, ..
        } => {
            let lhs = domain.read_value(*lhs)?;
            let rhs = domain.read_value(*rhs)?;
            domain.write_result(*result, lhs ^ rhs)?;
        }
        Bitwise::Not {
            operand, result, ..
        } => {
            let operand = domain.read_value(*operand)?;
            domain.write_result(*result, !operand)?;
        }
        Bitwise::Shl {
            lhs, rhs, result, ..
        } => {
            let lhs = domain.read_value(*lhs)?;
            let rhs = domain.read_value(*rhs)?;
            let value = lhs
                .checked_shl(rhs)
                .ok_or_else(|| D::Error::from(InterpreterError::Custom(Box::new(ShiftOverflow))))?;
            domain.write_result(*result, value)?;
        }
        Bitwise::Shr {
            lhs, rhs, result, ..
        } => {
            let lhs = domain.read_value(*lhs)?;
            let rhs = domain.read_value(*rhs)?;
            let value = lhs
                .checked_shr(rhs)
                .ok_or_else(|| D::Error::from(InterpreterError::Custom(Box::new(ShiftOverflow))))?;
            domain.write_result(*result, value)?;
        }
        Bitwise::__Phantom(..) => unreachable!(),
    }
    Ok(())
}

impl<'ir, S, L, V, C, T> Semantics<ConcreteInterp<'ir, S, L, V, C>> for Bitwise<T>
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
    V: Clone
        + BitAnd<Output = V>
        + BitOr<Output = V>
        + BitXor<Output = V>
        + Not<Output = V>
        + CheckedShl
        + CheckedShr,
    C: 'static,
    T: CompileTimeValue,
{
    type Effect = ();

    fn eval(&self, domain: &mut ConcreteInterp<'ir, S, L, V, C>) -> Result<(), InterpreterError> {
        eval_impl(self, domain)
    }
}

impl<'ir, S, L, V, T> Semantics<AbstractInterp<'ir, S, L, V>> for Bitwise<T>
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
    V: Clone
        + AbstractValue
        + BitAnd<Output = V>
        + BitOr<Output = V>
        + BitXor<Output = V>
        + Not<Output = V>
        + CheckedShl
        + CheckedShr,
    T: CompileTimeValue,
{
    type Effect = ();

    fn eval(&self, domain: &mut AbstractInterp<'ir, S, L, V>) -> Result<(), InterpreterError> {
        eval_impl(self, domain)
    }
}
