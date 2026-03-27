use kirin::prelude::{CompileTimeValue, PrettyPrint, Typeof};
use kirin_interpreter_2::{FromConstant, Interpretable, Interpreter, effect::Cursor};

use crate::Constant;

impl<'ir, I, T, Ty> Interpretable<'ir, I> for Constant<T, Ty>
where
    I: Interpreter<'ir> + kirin_interpreter_2::ValueStore<Error = <I as Interpreter<'ir>>::Error>,
    <I as kirin_interpreter_2::ValueStore>::Value: FromConstant<T>,
    T: CompileTimeValue + Typeof<Ty> + Clone + PrettyPrint,
    Ty: CompileTimeValue,
{
    type Effect = Cursor;
    type Error = <I as Interpreter<'ir>>::Error;

    fn interpret(&self, interp: &mut I) -> Result<Cursor, Self::Error> {
        let value =
            <I as kirin_interpreter_2::ValueStore>::Value::from_constant(self.value.clone())?;
        interp.write(self.result, value)?;
        Ok(Cursor::Advance)
    }
}
