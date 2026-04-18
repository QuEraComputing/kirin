use kirin::prelude::{CompileTimeValue, Dialect, HasStageInfo, StageMeta};
use kirin_interpreter::AbstractValue;
use kirin_interpreter_7::abstract_interp::AbstractInterp;
use kirin_interpreter_7::concrete::ConcreteInterp;
use kirin_interpreter_7::env::Interpretable;
use kirin_interpreter_7::error::InterpreterError;
use kirin_interpreter_7::store::Store;

use crate::{Cmp, CompareValue};

fn interp_impl<S, T>(op: &Cmp<T>, env: &mut S) -> Result<(), S::Error>
where
    S: Store,
    S::Value: CompareValue,
    <S::Value as CompareValue>::Bool: Into<S::Value>,
    T: CompileTimeValue,
{
    match op {
        Cmp::Eq {
            lhs, rhs, result, ..
        } => {
            let lhs = env.read(*lhs)?;
            let rhs = env.read(*rhs)?;
            env.write_result(*result, lhs.cmp_eq(&rhs).into())?;
        }
        Cmp::Ne {
            lhs, rhs, result, ..
        } => {
            let lhs = env.read(*lhs)?;
            let rhs = env.read(*rhs)?;
            env.write_result(*result, lhs.cmp_ne(&rhs).into())?;
        }
        Cmp::Lt {
            lhs, rhs, result, ..
        } => {
            let lhs = env.read(*lhs)?;
            let rhs = env.read(*rhs)?;
            env.write_result(*result, lhs.cmp_lt(&rhs).into())?;
        }
        Cmp::Le {
            lhs, rhs, result, ..
        } => {
            let lhs = env.read(*lhs)?;
            let rhs = env.read(*rhs)?;
            env.write_result(*result, lhs.cmp_le(&rhs).into())?;
        }
        Cmp::Gt {
            lhs, rhs, result, ..
        } => {
            let lhs = env.read(*lhs)?;
            let rhs = env.read(*rhs)?;
            env.write_result(*result, lhs.cmp_gt(&rhs).into())?;
        }
        Cmp::Ge {
            lhs, rhs, result, ..
        } => {
            let lhs = env.read(*lhs)?;
            let rhs = env.read(*rhs)?;
            env.write_result(*result, lhs.cmp_ge(&rhs).into())?;
        }
        Cmp::__Phantom(..) => unreachable!(),
    }
    Ok(())
}

impl<'ir, S, L, V, C, T> Interpretable<ConcreteInterp<'ir, S, L, V, C>> for Cmp<T>
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
    V: Clone + CompareValue,
    <V as CompareValue>::Bool: Into<V>,
    C: 'static,
    T: CompileTimeValue,
{
    type Effect = ();

    fn interpret(&self, env: &mut ConcreteInterp<'ir, S, L, V, C>) -> Result<(), InterpreterError> {
        interp_impl(self, env)
    }
}

impl<'ir, S, L, V, T> Interpretable<AbstractInterp<'ir, S, L, V>> for Cmp<T>
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
    V: Clone + AbstractValue + CompareValue,
    <V as CompareValue>::Bool: Into<V>,
    T: CompileTimeValue,
{
    type Effect = ();

    fn interpret(&self, env: &mut AbstractInterp<'ir, S, L, V>) -> Result<(), InterpreterError> {
        interp_impl(self, env)
    }
}
