use kirin::prelude::{CompileTimeValue, HasStageInfo, PrettyPrint, Typeof};
use kirin_interpreter::{Continuation, Interpretable, Interpreter, InterpreterError};

use crate::Constant;

impl<'ir, I, T, Ty> Interpretable<'ir, I> for Constant<T, Ty>
where
    I: Interpreter<'ir>,
    I::Value: TryFrom<T>,
    <I::Value as TryFrom<T>>::Error: std::error::Error + Send + Sync + 'static,
    T: CompileTimeValue + Typeof<Ty> + Clone + PrettyPrint,
    Ty: CompileTimeValue,
{
    fn interpret<L>(&self, interp: &mut I) -> Result<Continuation<I::Value, I::Ext>, I::Error>
    where
        I::StageInfo: HasStageInfo<L>,
        I::Error: From<InterpreterError>,
        L: Interpretable<'ir, I> + 'ir,
    {
        let val = I::Value::try_from(self.value.clone()).map_err(InterpreterError::custom)?;
        interp.write(self.result.into(), val)?;
        Ok(Continuation::Continue)
    }
}
