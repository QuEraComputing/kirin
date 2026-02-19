use kirin::prelude::{Dialect, HasStageInfo};
use kirin_interpreter::{Continuation, Interpretable, Interpreter, InterpreterError};

use crate::{FunctionBody, Return};

impl<I, L, T> Interpretable<I, L> for FunctionBody<T>
where
    I: Interpreter,
    I::StageInfo: HasStageInfo<L>,
    I::Error: From<InterpreterError>,
    L: Dialect,
    T: kirin::prelude::CompileTimeValue + Default,
{
    fn interpret(&self, interp: &mut I) -> Result<Continuation<I::Value, I::Ext>, I::Error> {
        let stage = interp.resolve_stage::<L>();
        let entry = self
            .body
            .blocks(stage)
            .next()
            .ok_or(InterpreterError::MissingEntry)?;
        Ok(Continuation::Jump(entry, vec![]))
    }
}

impl<I, L, T> Interpretable<I, L> for Return<T>
where
    I: Interpreter,
    I::Value: Clone,
    L: Dialect,
    T: kirin::prelude::CompileTimeValue + Default,
{
    fn interpret(&self, interp: &mut I) -> Result<Continuation<I::Value, I::Ext>, I::Error> {
        let v = interp.read(self.value)?;
        Ok(Continuation::Return(v))
    }
}
