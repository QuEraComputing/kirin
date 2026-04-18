use kirin::prelude::{CompileTimeValue, Dialect, HasStageInfo, StageMeta};
use kirin_interpreter::AbstractValue;
use kirin_interpreter_8::abstract_interp::AbstractInterp;
use kirin_interpreter_8::concrete::ConcreteInterp;
use kirin_interpreter_8::env::Env;
use kirin_interpreter_8::error::InterpreterError;
use kirin_interpreter_8::semantics::Semantics;

use crate::{Cmp, CompareValue};

fn eval_impl<D, T>(op: &Cmp<T>, domain: &mut D) -> Result<(), D::Error>
where
    D: Env,
    D::Value: CompareValue,
    <D::Value as CompareValue>::Bool: Into<D::Value>,
    T: CompileTimeValue,
{
    match op {
        Cmp::Eq {
            lhs, rhs, result, ..
        } => {
            let lhs = domain.read_value(*lhs)?;
            let rhs = domain.read_value(*rhs)?;
            domain.write_result(*result, lhs.cmp_eq(&rhs).into())?;
        }
        Cmp::Ne {
            lhs, rhs, result, ..
        } => {
            let lhs = domain.read_value(*lhs)?;
            let rhs = domain.read_value(*rhs)?;
            domain.write_result(*result, lhs.cmp_ne(&rhs).into())?;
        }
        Cmp::Lt {
            lhs, rhs, result, ..
        } => {
            let lhs = domain.read_value(*lhs)?;
            let rhs = domain.read_value(*rhs)?;
            domain.write_result(*result, lhs.cmp_lt(&rhs).into())?;
        }
        Cmp::Le {
            lhs, rhs, result, ..
        } => {
            let lhs = domain.read_value(*lhs)?;
            let rhs = domain.read_value(*rhs)?;
            domain.write_result(*result, lhs.cmp_le(&rhs).into())?;
        }
        Cmp::Gt {
            lhs, rhs, result, ..
        } => {
            let lhs = domain.read_value(*lhs)?;
            let rhs = domain.read_value(*rhs)?;
            domain.write_result(*result, lhs.cmp_gt(&rhs).into())?;
        }
        Cmp::Ge {
            lhs, rhs, result, ..
        } => {
            let lhs = domain.read_value(*lhs)?;
            let rhs = domain.read_value(*rhs)?;
            domain.write_result(*result, lhs.cmp_ge(&rhs).into())?;
        }
        Cmp::__Phantom(..) => unreachable!(),
    }
    Ok(())
}

impl<'ir, S, L, V, C, T> Semantics<ConcreteInterp<'ir, S, L, V, C>> for Cmp<T>
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
    V: Clone + CompareValue,
    <V as CompareValue>::Bool: Into<V>,
    C: 'static,
    T: CompileTimeValue,
{
    type Effect = ();

    fn eval(&self, domain: &mut ConcreteInterp<'ir, S, L, V, C>) -> Result<(), InterpreterError> {
        eval_impl(self, domain)
    }
}

impl<'ir, S, L, V, T> Semantics<AbstractInterp<'ir, S, L, V>> for Cmp<T>
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
    V: Clone + AbstractValue + CompareValue,
    <V as CompareValue>::Bool: Into<V>,
    T: CompileTimeValue,
{
    type Effect = ();

    fn eval(&self, domain: &mut AbstractInterp<'ir, S, L, V>) -> Result<(), InterpreterError> {
        eval_impl(self, domain)
    }
}
