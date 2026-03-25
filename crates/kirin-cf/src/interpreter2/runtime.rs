use kirin::prelude::{Block, CompileTimeValue, Dialect};
use kirin_interpreter_2::{
    ConsumeEffect, Interpretable, Interpreter, InterpreterError, ValueStore,
    interpreter::SingleStage,
};

pub trait Runtime<'ir, T>:
    Interpreter<'ir> + ValueStore<Error = <Self as Interpreter<'ir>>::Error>
where
    T: CompileTimeValue,
    <Self as ValueStore>::Value: Clone,
{
    fn bind_block_args(
        &mut self,
        block: Block,
        args: &[<Self as ValueStore>::Value],
    ) -> Result<(), <Self as Interpreter<'ir>>::Error>;
}

impl<'ir, L, T, V, M, E> Runtime<'ir, T> for SingleStage<'ir, L, V, M, E>
where
    L: Dialect + 'ir + Interpretable<'ir, SingleStage<'ir, L, V, M, E>, Machine = M>,
    T: CompileTimeValue,
    V: Clone + 'ir,
    M: kirin_interpreter_2::Machine<'ir> + ConsumeEffect<'ir> + 'ir,
    E: From<InterpreterError> + 'ir,
    <L as Interpretable<'ir, SingleStage<'ir, L, V, M, E>>>::Error: Into<E>,
    <M as ConsumeEffect<'ir>>::Error: Into<E>,
{
    fn bind_block_args(
        &mut self,
        block: Block,
        args: &[<Self as ValueStore>::Value],
    ) -> Result<(), <Self as Interpreter<'ir>>::Error> {
        Self::bind_block_args(self, block, args)
    }
}
