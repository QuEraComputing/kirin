use kirin_interpreter::{InterpretControl, Interpretable, Interpreter};

use crate::{FunctionBody, Return};

impl<I, T> Interpretable<I> for FunctionBody<T>
where
    I: Interpreter,
    T: kirin::prelude::CompileTimeValue + Default,
{
    fn interpret(&self, _interp: &mut I) -> Result<I::Control, I::Error> {
        Ok(I::Control::ctrl_continue())
    }
}

impl<I, T> Interpretable<I> for Return<T>
where
    I: Interpreter,
    I::Value: Clone,
    T: kirin::prelude::CompileTimeValue + Default,
{
    fn interpret(&self, interp: &mut I) -> Result<I::Control, I::Error> {
        let v = interp.read(self.value)?;
        Ok(I::Control::ctrl_return(v))
    }
}
