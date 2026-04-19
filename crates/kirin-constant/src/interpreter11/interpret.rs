use kirin::prelude::{CompileTimeValue, PrettyPrint, Typeof};
use kirin_interpreter_11::control::Control;
use kirin_interpreter_11::env::Env;
use kirin_interpreter_11::error::InterpreterError;
use kirin_interpreter_11::interpretable::Interpretable;

use crate::Constant;

impl<E, T, Ty> Interpretable<E> for Constant<T, Ty>
where
    E: Env,
    E::Value: TryFrom<T>,
    <E::Value as TryFrom<T>>::Error: std::error::Error + Send + Sync + 'static,
    E::Error: From<InterpreterError>,
    T: CompileTimeValue + Typeof<Ty> + Clone + PrettyPrint,
    Ty: CompileTimeValue,
{
    fn eval(&self, env: &mut E) -> Result<Control<E::Value, E::Ext>, E::Error> {
        let val = E::Value::try_from(self.value.clone())
            .map_err(|e| E::Error::from(InterpreterError::Custom(Box::new(e))))?;
        env.write_result(self.result, val)?;
        Ok(Control::Advance)
    }
}
