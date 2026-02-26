use kirin::prelude::{CompileTimeValue, Dialect, PrettyPrint, Typeof};
use kirin_interpreter::{Continuation, Interpretable, Interpreter, InterpreterError};

use crate::Constant;

impl<'ir, I, L, T, Ty> Interpretable<'ir, I, L> for Constant<T, Ty>
where
    I: Interpreter<'ir>,
    I::Value: From<T>,
    I::Error: From<InterpreterError>,
    L: Dialect,
    T: CompileTimeValue + Typeof<Ty> + Clone + PrettyPrint,
    Ty: CompileTimeValue + Default,
{
    fn interpret(&self, interp: &mut I) -> Result<Continuation<I::Value, I::Ext>, I::Error> {
        let val = I::Value::from(self.value.clone());
        interp.write(self.result, val)?;
        Ok(Continuation::Continue)
    }
}
