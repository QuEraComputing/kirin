use kirin::prelude::{CompileTimeValue, PrettyPrint, Typeof};
use kirin_interpreter_4::effect::CursorEffect;
use kirin_interpreter_4::error::InterpreterError;
use kirin_interpreter_4::lift::LiftInto;
use kirin_interpreter_4::traits::{Interpretable, Interpreter, Machine, ValueStore};

use crate::Constant;

impl<I, T, Ty> Interpretable<I> for Constant<T, Ty>
where
    I: Interpreter + Machine<Error = InterpreterError>,
    <I as ValueStore>::Value: TryFrom<T>,
    <<I as ValueStore>::Value as TryFrom<T>>::Error: std::error::Error + Send + Sync + 'static,
    CursorEffect<<I as ValueStore>::Value>: LiftInto<<I as Machine>::Effect>,
    T: CompileTimeValue + Typeof<Ty> + Clone + PrettyPrint,
    Ty: CompileTimeValue,
{
    type Effect = CursorEffect<<I as ValueStore>::Value>;
    type Error = InterpreterError;

    fn interpret(
        &self,
        interp: &mut I,
    ) -> Result<CursorEffect<<I as ValueStore>::Value>, InterpreterError> {
        let val = <I as ValueStore>::Value::try_from(self.value.clone())
            .map_err(|e| InterpreterError::Custom(Box::new(e)))?;
        interp.write(self.result, val)?;
        Ok(CursorEffect::Advance)
    }
}
