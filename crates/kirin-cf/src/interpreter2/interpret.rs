use kirin::prelude::{Block, CompileTimeValue, GetInfo, SSAValue};
use kirin_interpreter::BranchCondition;
use kirin_interpreter_2::{
    Interpretable, Interpreter, InterpreterError, ValueStore, effect::Cursor,
    interpreter::TypedStage,
};

use crate::ControlFlow;

fn unsupported(message: &'static str) -> kirin_interpreter_2::InterpreterError {
    kirin_interpreter_2::InterpreterError::custom(std::io::Error::other(message))
}

/// Eagerly bind block arguments using `ValueStore::write`.
///
/// Local helper replacing the removed `BlockBindings` trait. This will be
/// superseded by seed-carried args in a future task.
fn bind_block_args<'ir, I>(
    interp: &mut I,
    block: Block,
    args: impl IntoIterator<Item = <I as ValueStore>::Value>,
) -> Result<(), <I as Interpreter<'ir>>::Error>
where
    I: Interpreter<'ir> + TypedStage<'ir> + ValueStore<Error = <I as Interpreter<'ir>>::Error>,
    <I as ValueStore>::Value: Clone,
    <I as Interpreter<'ir>>::Error: From<InterpreterError>,
{
    let stage = interp.stage_info();
    let block_info = block.expect_info(stage);
    let expected = block_info.arguments.len();

    let mut got = 0;
    for (argument, value) in block_info.arguments.iter().zip(args) {
        interp.write(SSAValue::from(*argument), value)?;
        got += 1;
    }

    if got != expected {
        return Err(InterpreterError::ArityMismatch { expected, got }.into());
    }

    Ok(())
}

impl<'ir, I, T> Interpretable<'ir, I> for ControlFlow<T>
where
    I: Interpreter<'ir> + TypedStage<'ir> + ValueStore<Error = <I as Interpreter<'ir>>::Error>,
    <I as ValueStore>::Value: Clone + BranchCondition,
    <I as Interpreter<'ir>>::Error: From<InterpreterError>,
    T: CompileTimeValue,
{
    type Effect = Cursor<Block>;
    type Error = <I as Interpreter<'ir>>::Error;

    fn interpret(&self, interp: &mut I) -> Result<Cursor<Block>, Self::Error> {
        match self {
            ControlFlow::Branch { target, args } => {
                let values = interp.read_many(args)?;
                let block = target.target();
                bind_block_args(interp, block, values)?;
                Ok(Cursor::Jump(block))
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
                bind_block_args(interp, block, values)?;
                Ok(Cursor::Jump(block))
            }
            Self::__Phantom(..) => unreachable!(),
        }
    }
}
