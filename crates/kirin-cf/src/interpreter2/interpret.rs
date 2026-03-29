use kirin::prelude::CompileTimeValue;
use kirin_interpreter_2::{
    BlockSeed, BranchCondition, Interpretable, Interpreter, InterpreterError, ValueStore,
    effect::Cursor,
};

use crate::ControlFlow;

impl<'ir, I, T> Interpretable<'ir, I> for ControlFlow<T>
where
    I: Interpreter<'ir>,
    I::Value: Clone + BranchCondition,
    T: CompileTimeValue,
{
    type Effect = Cursor<BlockSeed<I::Value>>;
    type Error = <I as ValueStore>::Error;

    fn interpret(&self, interp: &mut I) -> Result<Cursor<BlockSeed<I::Value>>, Self::Error> {
        match self {
            ControlFlow::Branch { target, args } => {
                let values = interp.read_many(args)?;
                Ok(Cursor::jump(target.target(), values))
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
                        None => return Err(InterpreterError::unsupported(
                            "nondeterministic branch conditions are not supported in interpreter2",
                        )
                        .into()),
                    };
                let values = interp.read_many(args)?;
                Ok(Cursor::jump(block, values))
            }
            Self::__Phantom(..) => unreachable!(),
        }
    }
}
