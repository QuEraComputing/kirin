use kirin::prelude::CompileTimeValue;
use kirin_interpreter::BranchCondition;
use kirin_interpreter_2::{Interpretable, Interpreter, InterpreterError, ValueStore};

use crate::ControlFlow;

use super::{Effect, runtime::Runtime};

fn unsupported(message: &'static str) -> InterpreterError {
    InterpreterError::custom(std::io::Error::other(message))
}

impl<'ir, I, T> Interpretable<'ir, I> for ControlFlow<T>
where
    I: Runtime<'ir, T>,
    <I as ValueStore>::Value: Clone + BranchCondition,
    T: CompileTimeValue,
    <I as Interpreter<'ir>>::Error: From<InterpreterError>,
{
    type Machine = super::Machine;
    type Error = <I as Interpreter<'ir>>::Error;

    fn interpret(&self, interp: &mut I) -> Result<Effect, Self::Error> {
        match self {
            ControlFlow::Branch { target, args } => {
                let values = interp.read_many(args)?;
                let block = target.target();
                interp.bind_block_args(block, &values)?;
                Ok(Effect::Jump(block.into()))
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
                interp.bind_block_args(block, &values)?;
                Ok(Effect::Jump(block.into()))
            }
            Self::__Phantom(..) => unreachable!(),
        }
    }
}
