use kirin::prelude::{CompileTimeValue, Dialect, HasStageInfo, PrettyPrint, StageMeta, Typeof};
use kirin_interpreter::AbstractValue;
use kirin_interpreter_7::abstract_interp::AbstractInterp;
use kirin_interpreter_7::concrete::ConcreteInterp;
use kirin_interpreter_7::env::Interpretable;
use kirin_interpreter_7::error::InterpreterError;
use kirin_interpreter_7::store::Store;

use crate::Constant;

fn interp_impl<S, T, Ty>(op: &Constant<T, Ty>, env: &mut S) -> Result<(), S::Error>
where
    S: Store,
    S::Value: TryFrom<T>,
    <S::Value as TryFrom<T>>::Error: std::error::Error + Send + Sync + 'static,
    S::Error: From<InterpreterError>,
    T: CompileTimeValue + Typeof<Ty> + Clone + PrettyPrint,
    Ty: CompileTimeValue,
{
    let val = S::Value::try_from(op.value.clone())
        .map_err(|e| S::Error::from(InterpreterError::Custom(Box::new(e))))?;
    env.write_result(op.result, val)?;
    Ok(())
}

impl<'ir, S, L, V, C, T, Ty> Interpretable<ConcreteInterp<'ir, S, L, V, C>> for Constant<T, Ty>
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

    fn interpret(&self, env: &mut ConcreteInterp<'ir, S, L, V, C>) -> Result<(), InterpreterError> {
        interp_impl(self, env)
    }
}

impl<'ir, S, L, V, T, Ty> Interpretable<AbstractInterp<'ir, S, L, V>> for Constant<T, Ty>
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
    V: Clone + AbstractValue + TryFrom<T>,
    <V as TryFrom<T>>::Error: std::error::Error + Send + Sync + 'static,
    T: CompileTimeValue + Typeof<Ty> + Clone + PrettyPrint,
    Ty: CompileTimeValue,
{
    type Effect = ();

    fn interpret(&self, env: &mut AbstractInterp<'ir, S, L, V>) -> Result<(), InterpreterError> {
        interp_impl(self, env)
    }
}
