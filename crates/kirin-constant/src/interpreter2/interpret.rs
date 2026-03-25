use kirin::prelude::{CompileTimeValue, PrettyPrint, Typeof};
use kirin_interpreter_2::{Interpretable, Interpreter, InterpreterError, ValueStore};

use crate::Constant;

use super::Effect;

impl<'ir, I, T, Ty> Interpretable<'ir, I> for Constant<T, Ty>
where
    I: Interpreter<'ir> + ValueStore<Error = <I as Interpreter<'ir>>::Error>,
    <I as ValueStore>::Value: TryFrom<T>,
    <<I as ValueStore>::Value as TryFrom<T>>::Error: std::error::Error + Send + Sync + 'static,
    T: CompileTimeValue + Typeof<Ty> + Clone + PrettyPrint,
    Ty: CompileTimeValue,
    <I as Interpreter<'ir>>::Error: From<InterpreterError>,
{
    type Machine = super::Machine;
    type Error = <I as Interpreter<'ir>>::Error;

    fn interpret(&self, interp: &mut I) -> Result<Effect, Self::Error> {
        let value = <I as ValueStore>::Value::try_from(self.value.clone())
            .map_err(InterpreterError::custom)?;
        interp.write(self.result, value)?;
        Ok(Effect::Advance)
    }
}
