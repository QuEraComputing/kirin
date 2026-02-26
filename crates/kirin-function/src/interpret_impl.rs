use kirin::prelude::{Dialect, HasStageInfo};
use kirin_interpreter::smallvec::smallvec;
use kirin_interpreter::{Continuation, Interpretable, Interpreter, InterpreterError, SSACFGRegion};

use crate::{FunctionBody, Return};

impl<T> SSACFGRegion for FunctionBody<T>
where
    T: kirin::prelude::CompileTimeValue + Default,
{
    fn entry_block<L: Dialect>(
        &self,
        stage: &kirin::prelude::StageInfo<L>,
    ) -> Result<kirin::prelude::Block, InterpreterError> {
        self.body
            .blocks(stage)
            .next()
            .ok_or(InterpreterError::MissingEntry)
    }
}

impl<'ir, I, L, T> Interpretable<'ir, I, L> for FunctionBody<T>
where
    I: Interpreter<'ir>,
    I::StageInfo: HasStageInfo<L>,
    I::Error: From<InterpreterError>,
    L: Dialect + 'ir,
    T: kirin::prelude::CompileTimeValue + Default,
{
    fn interpret(&self, interp: &mut I) -> Result<Continuation<I::Value, I::Ext>, I::Error> {
        let stage = interp.active_stage_info::<L>();
        let entry = self
            .body
            .blocks(stage)
            .next()
            .ok_or(InterpreterError::MissingEntry)?;
        Ok(Continuation::Jump(
            kirin::prelude::Successor::from_block(entry),
            smallvec![],
        ))
    }
}

impl<'ir, I, L, T> Interpretable<'ir, I, L> for Return<T>
where
    I: Interpreter<'ir>,
    I::Value: Clone,
    L: Dialect,
    T: kirin::prelude::CompileTimeValue + Default,
{
    fn interpret(&self, interp: &mut I) -> Result<Continuation<I::Value, I::Ext>, I::Error> {
        let v = interp.read(self.value)?;
        Ok(Continuation::Return(v))
    }
}
