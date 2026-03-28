use kirin::prelude::CompileTimeValue;
use kirin_interpreter::BranchCondition;
use kirin_interpreter_2::{Interpretable, Interpreter, effect::Cursor, interpreter::BlockBindings};

use crate::ControlFlow;

fn unsupported(message: &'static str) -> kirin_interpreter_2::InterpreterError {
    kirin_interpreter_2::InterpreterError::custom(std::io::Error::other(message))
}

impl<'ir, I, T> Interpretable<'ir, I> for ControlFlow<T>
where
    I: BlockBindings<'ir> + kirin_interpreter_2::ValueStore<Error = <I as Interpreter<'ir>>::Error>,
    <I as kirin_interpreter_2::ValueStore>::Value: Clone + BranchCondition,
    T: CompileTimeValue,
{
    type Effect = Cursor;
    type Error = <I as Interpreter<'ir>>::Error;

    fn interpret(&self, interp: &mut I) -> Result<Cursor, Self::Error> {
        match self {
            ControlFlow::Branch { target, args } => {
                let values = interp.read_many(args)?;
                let block = target.target();
                interp.bind_block_args(block, values)?;
                Ok(Cursor::Jump(block.into()))
            }
            ControlFlow::ConditionalBranch {
                condition,
                true_target,
                true_args,
                false_target,
                false_args,
            } => {
                let cond = interp.read(*condition)?;
                let (block, args) =
                    match cond.is_truthy() {
                        Some(true) => (true_target.target(), true_args.as_slice()),
                        Some(false) => (false_target.target(), false_args.as_slice()),
                        None => return Err(unsupported(
                            "nondeterministic branch conditions are not supported in interpreter2",
                        )
                        .into()),
                    };
                let values = interp.read_many(args)?;
                interp.bind_block_args(block, values)?;
                Ok(Cursor::Jump(block.into()))
            }
            Self::__Phantom(..) => unreachable!(),
        }
    }
}
