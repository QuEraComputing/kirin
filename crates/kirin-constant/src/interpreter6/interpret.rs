use kirin::prelude::{CompileTimeValue, PrettyPrint, Typeof};
use kirin_interpreter_6::env::{Env, Interpretable};
use kirin_interpreter_6::error::InterpreterError;
use kirin_interpreter_6::lift::Lift;

use crate::Constant;

impl<E, T, Ty> Interpretable<E> for Constant<T, Ty>
where
    E: Env,
    E::Effect: Lift<()>,
    E::Value: TryFrom<T>,
    <E::Value as TryFrom<T>>::Error: std::error::Error + Send + Sync + 'static,
    T: CompileTimeValue + Typeof<Ty> + Clone + PrettyPrint,
    Ty: CompileTimeValue,
{
    type DialectEffect = E::Effect;

    fn interpret(&self, env: &mut E) -> Result<E::Effect, E::Error> {
        let val = E::Value::try_from(self.value.clone())
            .map_err(|e| E::Error::from(InterpreterError::Custom(Box::new(e))))?;
        env.write(self.result, val)?;
        Ok(E::advance())
    }
}
