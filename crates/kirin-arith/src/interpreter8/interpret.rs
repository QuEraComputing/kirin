use std::ops::{Add, Mul, Neg, Sub};

use kirin::prelude::{CompileTimeValue, Dialect, HasStageInfo, StageMeta};
use kirin_interpreter::AbstractValue;
use kirin_interpreter_8::abstract_interp::AbstractInterp;
use kirin_interpreter_8::concrete::ConcreteInterp;
use kirin_interpreter_8::env::Env;
use kirin_interpreter_8::error::InterpreterError;
use kirin_interpreter_8::semantics::Semantics;

use crate::{Arith, CheckedDiv, CheckedRem};

#[derive(Debug)]
struct DivisionByZero;

impl std::fmt::Display for DivisionByZero {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "division by zero")
    }
}

impl std::error::Error for DivisionByZero {}

fn eval_impl<D, T>(op: &Arith<T>, domain: &mut D) -> Result<(), D::Error>
where
    D: Env,
    D::Value: Clone
        + Add<Output = D::Value>
        + Sub<Output = D::Value>
        + Mul<Output = D::Value>
        + Neg<Output = D::Value>
        + CheckedDiv
        + CheckedRem,
    D::Error: From<InterpreterError>,
    T: CompileTimeValue,
{
    match op {
        Arith::Add {
            lhs, rhs, result, ..
        } => {
            let l = domain.read_value(*lhs)?;
            let r = domain.read_value(*rhs)?;
            domain.write_result(*result, l + r)?;
        }
        Arith::Sub {
            lhs, rhs, result, ..
        } => {
            let l = domain.read_value(*lhs)?;
            let r = domain.read_value(*rhs)?;
            domain.write_result(*result, l - r)?;
        }
        Arith::Mul {
            lhs, rhs, result, ..
        } => {
            let l = domain.read_value(*lhs)?;
            let r = domain.read_value(*rhs)?;
            domain.write_result(*result, l * r)?;
        }
        Arith::Div {
            lhs, rhs, result, ..
        } => {
            let l = domain.read_value(*lhs)?;
            let r = domain.read_value(*rhs)?;
            let v = l.checked_div(r).ok_or_else(|| {
                D::Error::from(InterpreterError::Custom(Box::new(DivisionByZero)))
            })?;
            domain.write_result(*result, v)?;
        }
        Arith::Rem {
            lhs, rhs, result, ..
        } => {
            let l = domain.read_value(*lhs)?;
            let r = domain.read_value(*rhs)?;
            let v = l.checked_rem(r).ok_or_else(|| {
                D::Error::from(InterpreterError::Custom(Box::new(DivisionByZero)))
            })?;
            domain.write_result(*result, v)?;
        }
        Arith::Neg {
            operand, result, ..
        } => {
            let v = domain.read_value(*operand)?;
            domain.write_result(*result, -v)?;
        }
        Arith::__Phantom(..) => unreachable!(),
    }
    Ok(())
}

impl<'ir, S, L, V, C, T> Semantics<ConcreteInterp<'ir, S, L, V, C>> for Arith<T>
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
    V: Clone
        + Add<Output = V>
        + Sub<Output = V>
        + Mul<Output = V>
        + Neg<Output = V>
        + CheckedDiv
        + CheckedRem,
    C: 'static,
    T: CompileTimeValue,
{
    type Effect = ();

    fn eval(&self, domain: &mut ConcreteInterp<'ir, S, L, V, C>) -> Result<(), InterpreterError> {
        eval_impl(self, domain)
    }
}

impl<'ir, S, L, V, T> Semantics<AbstractInterp<'ir, S, L, V>> for Arith<T>
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
    V: Clone
        + AbstractValue
        + Add<Output = V>
        + Sub<Output = V>
        + Mul<Output = V>
        + Neg<Output = V>
        + CheckedDiv
        + CheckedRem,
    T: CompileTimeValue,
{
    type Effect = ();

    fn eval(&self, domain: &mut AbstractInterp<'ir, S, L, V>) -> Result<(), InterpreterError> {
        eval_impl(self, domain)
    }
}
