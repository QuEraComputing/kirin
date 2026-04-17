use kirin::prelude::CompileTimeValue;
use kirin_interpreter_5::env::{Env, Interpretable};

use crate::{Cmp, CompareValue};

impl<D, T> Interpretable<D> for Cmp<T>
where
    D: Env,
    D::Value: CompareValue,
    <D::Value as CompareValue>::Bool: Into<D::Value>,
    T: CompileTimeValue,
{
    fn interpret(&self, domain: &mut D) -> Result<D::Effect, D::Error> {
        match self {
            Cmp::Eq {
                lhs, rhs, result, ..
            } => {
                let lhs = domain.read(*lhs)?;
                let rhs = domain.read(*rhs)?;
                domain.write(*result, lhs.cmp_eq(&rhs).into())?;
            }
            Cmp::Ne {
                lhs, rhs, result, ..
            } => {
                let lhs = domain.read(*lhs)?;
                let rhs = domain.read(*rhs)?;
                domain.write(*result, lhs.cmp_ne(&rhs).into())?;
            }
            Cmp::Lt {
                lhs, rhs, result, ..
            } => {
                let lhs = domain.read(*lhs)?;
                let rhs = domain.read(*rhs)?;
                domain.write(*result, lhs.cmp_lt(&rhs).into())?;
            }
            Cmp::Le {
                lhs, rhs, result, ..
            } => {
                let lhs = domain.read(*lhs)?;
                let rhs = domain.read(*rhs)?;
                domain.write(*result, lhs.cmp_le(&rhs).into())?;
            }
            Cmp::Gt {
                lhs, rhs, result, ..
            } => {
                let lhs = domain.read(*lhs)?;
                let rhs = domain.read(*rhs)?;
                domain.write(*result, lhs.cmp_gt(&rhs).into())?;
            }
            Cmp::Ge {
                lhs, rhs, result, ..
            } => {
                let lhs = domain.read(*lhs)?;
                let rhs = domain.read(*rhs)?;
                domain.write(*result, lhs.cmp_ge(&rhs).into())?;
            }
            Self::__Phantom(..) => unreachable!(),
        }
        Ok(D::advance())
    }
}
