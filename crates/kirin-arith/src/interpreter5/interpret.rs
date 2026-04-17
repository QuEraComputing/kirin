use std::ops::{Add, Mul, Neg, Sub};

use kirin::prelude::CompileTimeValue;
use kirin_interpreter_5::env::{Env, Interpretable};
use kirin_interpreter_5::error::InterpreterError;

use crate::{Arith, CheckedDiv, CheckedRem};

#[derive(Debug)]
struct DivisionByZero;

impl std::fmt::Display for DivisionByZero {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "division by zero")
    }
}

impl std::error::Error for DivisionByZero {}

impl<D, T> Interpretable<D> for Arith<T>
where
    D: Env,
    D::Value: Clone
        + Add<Output = D::Value>
        + Sub<Output = D::Value>
        + Mul<Output = D::Value>
        + Neg<Output = D::Value>
        + CheckedDiv
        + CheckedRem,
    T: CompileTimeValue,
{
    fn interpret(&self, domain: &mut D) -> Result<D::Effect, D::Error> {
        match self {
            Arith::Add {
                lhs, rhs, result, ..
            } => {
                let l = domain.read(*lhs)?;
                let r = domain.read(*rhs)?;
                domain.write(*result, l + r)?;
            }
            Arith::Sub {
                lhs, rhs, result, ..
            } => {
                let l = domain.read(*lhs)?;
                let r = domain.read(*rhs)?;
                domain.write(*result, l - r)?;
            }
            Arith::Mul {
                lhs, rhs, result, ..
            } => {
                let l = domain.read(*lhs)?;
                let r = domain.read(*rhs)?;
                domain.write(*result, l * r)?;
            }
            Arith::Div {
                lhs, rhs, result, ..
            } => {
                let l = domain.read(*lhs)?;
                let r = domain.read(*rhs)?;
                let v = l.checked_div(r).ok_or_else(|| {
                    D::Error::from(InterpreterError::Custom(Box::new(DivisionByZero)))
                })?;
                domain.write(*result, v)?;
            }
            Arith::Rem {
                lhs, rhs, result, ..
            } => {
                let l = domain.read(*lhs)?;
                let r = domain.read(*rhs)?;
                let v = l.checked_rem(r).ok_or_else(|| {
                    D::Error::from(InterpreterError::Custom(Box::new(DivisionByZero)))
                })?;
                domain.write(*result, v)?;
            }
            Arith::Neg {
                operand, result, ..
            } => {
                let o = domain.read(*operand)?;
                domain.write(*result, -o)?;
            }
            Self::__Phantom(..) => unreachable!(),
        }
        Ok(D::advance())
    }
}
