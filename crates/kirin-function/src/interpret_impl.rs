use kirin_interpreter::{Continuation, Interpretable, Interpreter};

use crate::{FunctionBody, Return};

impl<I, T> Interpretable<I> for FunctionBody<T>
where
    I: Interpreter,
    T: kirin::prelude::CompileTimeValue + Default,
{
    fn interpret(&self, _interp: &mut I) -> Result<Continuation<I::Value, I::Ext>, I::Error> {
        Ok(Continuation::Continue)
    }
}

impl<I, T> Interpretable<I> for Return<T>
where
    I: Interpreter,
    I::Value: Clone,
    T: kirin::prelude::CompileTimeValue + Default,
{
    fn interpret(&self, interp: &mut I) -> Result<Continuation<I::Value, I::Ext>, I::Error> {
        let v = interp.read(self.value)?;
        Ok(Continuation::Return(v))
    }
}
