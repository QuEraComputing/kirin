use kirin::prelude::{CompileTimeValue, PrettyPrint, Typeof};
use kirin_interpreter_5::env::{Env, Interpretable};
use kirin_interpreter_5::error::InterpreterError;

use crate::Constant;

impl<D, T, Ty> Interpretable<D> for Constant<T, Ty>
where
    D: Env,
    D::Value: TryFrom<T>,
    <D::Value as TryFrom<T>>::Error: std::error::Error + Send + Sync + 'static,
    T: CompileTimeValue + Typeof<Ty> + Clone + PrettyPrint,
    Ty: CompileTimeValue,
{
    fn interpret(&self, domain: &mut D) -> Result<D::Effect, D::Error> {
        let val = D::Value::try_from(self.value.clone())
            .map_err(|e| D::Error::from(InterpreterError::Custom(Box::new(e))))?;
        domain.write(self.result, val)?;
        Ok(D::advance())
    }
}
