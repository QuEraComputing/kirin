use kirin_ir::{Block, GetInfo, SSAValue};

use super::{Interpreter, TypedStage};
use crate::{InterpreterError, ValueStore};

/// Default block-argument binding for typed stage-local shells.
pub trait BlockBindings<'ir>:
    Interpreter<'ir> + TypedStage<'ir> + ValueStore<Error = <Self as Interpreter<'ir>>::Error>
where
    <Self as ValueStore>::Value: Clone,
    <Self as Interpreter<'ir>>::Error: From<InterpreterError>,
{
    fn bind_block_args(
        &mut self,
        block: Block,
        args: &[<Self as ValueStore>::Value],
    ) -> Result<(), <Self as Interpreter<'ir>>::Error> {
        let stage = self.stage_info();
        let block_info = block.expect_info(stage);
        if block_info.arguments.len() != args.len() {
            return Err(InterpreterError::ArityMismatch {
                expected: block_info.arguments.len(),
                got: args.len(),
            }
            .into());
        }

        for (argument, value) in block_info.arguments.iter().zip(args.iter()) {
            self.write(SSAValue::from(*argument), value.clone())?;
        }

        Ok(())
    }
}

impl<'ir, T> BlockBindings<'ir> for T
where
    T: Interpreter<'ir> + TypedStage<'ir> + ValueStore<Error = <T as Interpreter<'ir>>::Error>,
    <T as ValueStore>::Value: Clone,
    <T as Interpreter<'ir>>::Error: From<InterpreterError>,
{
}
