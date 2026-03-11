use kirin::prelude::{CompileTimeValue, Dialect, HasStageInfo, PrettyPrint, Typeof};
use kirin_interpreter::{Continuation, Interpretable, Interpreter, InterpreterError};

use crate::Constant;

impl<'ir, I, T, Ty> Interpretable<'ir, I> for Constant<T, Ty>
where
    I: Interpreter<'ir>,
    I::Value: From<T>,
    I::Error: From<InterpreterError>,
    T: CompileTimeValue + Typeof<Ty> + Clone + PrettyPrint,
    Ty: CompileTimeValue,
{
    fn interpret<L: Dialect>(
        &self,
        interp: &mut I,
    ) -> Result<Continuation<I::Value, I::Ext>, I::Error>
    where
        I::StageInfo: HasStageInfo<L>,
        I::Error: From<InterpreterError>,
        L: Interpretable<'ir, I> + 'ir,
    {
        let val = I::Value::from(self.value.clone());
        interp.write(self.result, val)?;
        Ok(Continuation::Continue)
    }
}
