use kirin::prelude::{CompileTimeValue, Dialect};
use kirin_interpreter::{Continuation, Interpretable, Interpreter, InterpreterError};

use crate::Cmp;

/// Comparison operations producing a value of the same type.
///
/// Returns 1 for true, 0 for false in concrete interpreters.
/// Abstract interpreters may return over-approximations.
pub trait CompareValue {
    fn cmp_eq(&self, other: &Self) -> Self;
    fn cmp_ne(&self, other: &Self) -> Self;
    fn cmp_lt(&self, other: &Self) -> Self;
    fn cmp_le(&self, other: &Self) -> Self;
    fn cmp_gt(&self, other: &Self) -> Self;
    fn cmp_ge(&self, other: &Self) -> Self;
}

impl CompareValue for i64 {
    fn cmp_eq(&self, other: &Self) -> Self {
        if self == other { 1 } else { 0 }
    }
    fn cmp_ne(&self, other: &Self) -> Self {
        if self != other { 1 } else { 0 }
    }
    fn cmp_lt(&self, other: &Self) -> Self {
        if self < other { 1 } else { 0 }
    }
    fn cmp_le(&self, other: &Self) -> Self {
        if self <= other { 1 } else { 0 }
    }
    fn cmp_gt(&self, other: &Self) -> Self {
        if self > other { 1 } else { 0 }
    }
    fn cmp_ge(&self, other: &Self) -> Self {
        if self >= other { 1 } else { 0 }
    }
}

impl<I, L, T> Interpretable<I, L> for Cmp<T>
where
    I: Interpreter,
    I::Value: CompareValue,
    I::Error: From<InterpreterError>,
    L: Dialect,
    T: CompileTimeValue + Default,
{
    fn interpret(&self, interp: &mut I) -> Result<Continuation<I::Value, I::Ext>, I::Error> {
        match self {
            Cmp::Eq {
                lhs, rhs, result, ..
            } => {
                let a = interp.read(*lhs)?;
                let b = interp.read(*rhs)?;
                interp.write(*result, a.cmp_eq(&b))?;
                Ok(Continuation::Continue)
            }
            Cmp::Ne {
                lhs, rhs, result, ..
            } => {
                let a = interp.read(*lhs)?;
                let b = interp.read(*rhs)?;
                interp.write(*result, a.cmp_ne(&b))?;
                Ok(Continuation::Continue)
            }
            Cmp::Lt {
                lhs, rhs, result, ..
            } => {
                let a = interp.read(*lhs)?;
                let b = interp.read(*rhs)?;
                interp.write(*result, a.cmp_lt(&b))?;
                Ok(Continuation::Continue)
            }
            Cmp::Le {
                lhs, rhs, result, ..
            } => {
                let a = interp.read(*lhs)?;
                let b = interp.read(*rhs)?;
                interp.write(*result, a.cmp_le(&b))?;
                Ok(Continuation::Continue)
            }
            Cmp::Gt {
                lhs, rhs, result, ..
            } => {
                let a = interp.read(*lhs)?;
                let b = interp.read(*rhs)?;
                interp.write(*result, a.cmp_gt(&b))?;
                Ok(Continuation::Continue)
            }
            Cmp::Ge {
                lhs, rhs, result, ..
            } => {
                let a = interp.read(*lhs)?;
                let b = interp.read(*rhs)?;
                interp.write(*result, a.cmp_ge(&b))?;
                Ok(Continuation::Continue)
            }
        }
    }
}
