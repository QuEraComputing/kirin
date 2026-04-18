use kirin::prelude::{CompileTimeValue, Dialect, HasStageInfo, PrettyPrint, StageMeta, Typeof};
use kirin_interpreter::AbstractValue;
use kirin_interpreter_8::abstract_interp::AbstractInterp;
use kirin_interpreter_8::concrete::ConcreteInterp;
use kirin_interpreter_8::env::Env;
use kirin_interpreter_8::error::InterpreterError;
use kirin_interpreter_8::semantics::Semantics;

use crate::Constant;

fn eval_impl<D, T, Ty>(op: &Constant<T, Ty>, domain: &mut D) -> Result<(), D::Error>
where
    D: Env,
    D::Value: TryFrom<T>,
    <D::Value as TryFrom<T>>::Error: std::error::Error + Send + Sync + 'static,
    D::Error: From<InterpreterError>,
    T: CompileTimeValue + Typeof<Ty> + Clone + PrettyPrint,
    Ty: CompileTimeValue,
{
    let val = D::Value::try_from(op.value.clone())
        .map_err(|e| D::Error::from(InterpreterError::Custom(Box::new(e))))?;
    domain.write_result(op.result, val)?;
    Ok(())
}

impl<'ir, S, L, V, C, T, Ty> Semantics<ConcreteInterp<'ir, S, L, V, C>> for Constant<T, Ty>
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
    V: Clone + TryFrom<T>,
    <V as TryFrom<T>>::Error: std::error::Error + Send + Sync + 'static,
    C: 'static,
    T: CompileTimeValue + Typeof<Ty> + Clone + PrettyPrint,
    Ty: CompileTimeValue,
{
    type Effect = ();

    fn eval(&self, domain: &mut ConcreteInterp<'ir, S, L, V, C>) -> Result<(), InterpreterError> {
        eval_impl(self, domain)
    }
}

impl<'ir, S, L, V, T, Ty> Semantics<AbstractInterp<'ir, S, L, V>> for Constant<T, Ty>
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
    V: Clone + AbstractValue + TryFrom<T>,
    <V as TryFrom<T>>::Error: std::error::Error + Send + Sync + 'static,
    T: CompileTimeValue + Typeof<Ty> + Clone + PrettyPrint,
    Ty: CompileTimeValue,
{
    type Effect = ();

    fn eval(&self, domain: &mut AbstractInterp<'ir, S, L, V>) -> Result<(), InterpreterError> {
        eval_impl(self, domain)
    }
}
