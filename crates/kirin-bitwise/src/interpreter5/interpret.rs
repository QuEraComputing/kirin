use std::ops::{BitAnd, BitOr, BitXor, Not};

use kirin::prelude::CompileTimeValue;
use kirin_interpreter_5::env::{Env, Interpretable};
use kirin_interpreter_5::error::InterpreterError;

use crate::{Bitwise, CheckedShl, CheckedShr};

#[derive(Debug)]
struct ShiftOverflow;

impl std::fmt::Display for ShiftOverflow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "shift overflow")
    }
}

impl std::error::Error for ShiftOverflow {}

impl<D, T> Interpretable<D> for Bitwise<T>
where
    D: Env,
    D::Value: Clone
        + BitAnd<Output = D::Value>
        + BitOr<Output = D::Value>
        + BitXor<Output = D::Value>
        + Not<Output = D::Value>
        + CheckedShl
        + CheckedShr,
    T: CompileTimeValue,
{
    fn interpret(&self, domain: &mut D) -> Result<D::Effect, D::Error> {
        match self {
            Bitwise::And {
                lhs, rhs, result, ..
            } => {
                let lhs = domain.read(*lhs)?;
                let rhs = domain.read(*rhs)?;
                domain.write(*result, lhs & rhs)?;
            }
            Bitwise::Or {
                lhs, rhs, result, ..
            } => {
                let lhs = domain.read(*lhs)?;
                let rhs = domain.read(*rhs)?;
                domain.write(*result, lhs | rhs)?;
            }
            Bitwise::Xor {
                lhs, rhs, result, ..
            } => {
                let lhs = domain.read(*lhs)?;
                let rhs = domain.read(*rhs)?;
                domain.write(*result, lhs ^ rhs)?;
            }
            Bitwise::Not {
                operand, result, ..
            } => {
                let operand = domain.read(*operand)?;
                domain.write(*result, !operand)?;
            }
            Bitwise::Shl {
                lhs, rhs, result, ..
            } => {
                let lhs = domain.read(*lhs)?;
                let rhs = domain.read(*rhs)?;
                let value = lhs.checked_shl(rhs).ok_or_else(|| {
                    D::Error::from(InterpreterError::Custom(Box::new(ShiftOverflow)))
                })?;
                domain.write(*result, value)?;
            }
            Bitwise::Shr {
                lhs, rhs, result, ..
            } => {
                let lhs = domain.read(*lhs)?;
                let rhs = domain.read(*rhs)?;
                let value = lhs.checked_shr(rhs).ok_or_else(|| {
                    D::Error::from(InterpreterError::Custom(Box::new(ShiftOverflow)))
                })?;
                domain.write(*result, value)?;
            }
            Self::__Phantom(..) => unreachable!(),
        }
        Ok(D::advance())
    }
}
