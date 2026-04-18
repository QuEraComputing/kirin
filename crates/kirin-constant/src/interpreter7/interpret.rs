use kirin::prelude::{CompileTimeValue, PrettyPrint, Typeof};
use kirin_interpreter_7::env::{Interp, Interpretable};
use kirin_interpreter_7::error::InterpreterError;

use crate::Constant;

/// Pure value op: returns `()` (advance).
impl<E, T, Ty> Interpretable<E> for Constant<T, Ty>
where
    E: Interp,
    E::Value: TryFrom<T>,
    <E::Value as TryFrom<T>>::Error: std::error::Error + Send + Sync + 'static,
    T: CompileTimeValue + Typeof<Ty> + Clone + PrettyPrint,
    Ty: CompileTimeValue,
{
    type Effect = ();

    fn interpret(&self, env: &mut E) -> Result<(), E::Error> {
        let val = E::Value::try_from(self.value.clone())
            .map_err(|e| E::Error::from(InterpreterError::Custom(Box::new(e))))?;
        env.write_result(self.result, val)?;
        Ok(())
    }
}
