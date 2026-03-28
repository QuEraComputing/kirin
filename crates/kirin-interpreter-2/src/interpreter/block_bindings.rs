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
        args: impl IntoIterator<Item = <Self as ValueStore>::Value>,
    ) -> Result<(), <Self as Interpreter<'ir>>::Error> {
        let stage = self.stage_info();
        let block_info = block.expect_info(stage);
        let expected = block_info.arguments.len();

        let mut got = 0;
        for (argument, value) in block_info.arguments.iter().zip(args) {
            self.write(SSAValue::from(*argument), value)?;
            got += 1;
        }

        if got != expected {
            return Err(InterpreterError::ArityMismatch { expected, got }.into());
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
