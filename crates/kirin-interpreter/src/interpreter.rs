use std::fmt;

use kirin_ir::{
    Block, Dialect, GetInfo, HasStageInfo, SSAValue, StageInfo,
};

use crate::Continuation;
use crate::InterpreterError;
use crate::ValueStore;
use crate::stage_access::StageAccess;

/// Minimal state contract for interpreter implementations.
///
/// Requires [`ValueStore`] for SSA value read/write and [`StageAccess`] for
/// pipeline / stage resolution. The associated `Ext` type determines which
/// extra continuation variants are available -- concrete interpreters use
/// [`crate::ConcreteExt`] while abstract interpreters use
/// [`std::convert::Infallible`].
///
/// Call semantics are inherent on each interpreter type
/// (e.g. [`crate::StackInterpreter::call`],
/// [`crate::AbstractInterpreter::analyze`]).
pub trait Interpreter<'ir>: ValueStore + StageAccess<'ir> + 'ir {
    type Ext: fmt::Debug;

    /// Bind values to a block's arguments in the current frame.
    ///
    /// Resolves the block's argument SSA values from stage info and writes
    /// each provided value. Returns `ArityMismatch` if `args.len()` differs
    /// from the block's declared argument count.
    fn bind_block_args<L: Dialect>(
        &mut self,
        stage: &'ir StageInfo<L>,
        block: Block,
        args: &[Self::Value],
    ) -> Result<(), Self::Error>
    where
        Self::Error: From<InterpreterError>,
    {
        let block_info = block.expect_info(stage);
        if block_info.arguments.len() != args.len() {
            return Err(InterpreterError::ArityMismatch {
                expected: block_info.arguments.len(),
                got: args.len(),
            }
            .into());
        }
        let arg_ssas: Vec<SSAValue> = block_info
            .arguments
            .iter()
            .map(|ba| SSAValue::from(*ba))
            .collect();
        for (ssa, val) in arg_ssas.iter().zip(args.iter()) {
            self.write_ssa(*ssa, val.clone())?;
        }
        Ok(())
    }

    /// Execute a body block whose arguments have already been bound.
    ///
    /// Returns a [`Continuation`] representing the block's result. The
    /// concrete variant depends on the interpreter: `StackInterpreter`
    /// always returns `Continuation::Yield(values)` (using cursor-based
    /// execution internally), while other implementations may propagate
    /// the terminator's continuation directly.
    ///
    /// The caller must call [`bind_block_args`](Self::bind_block_args) first
    /// to write values into the block's argument SSA slots.
    fn eval_block<L: Dialect>(
        &mut self,
        stage: &'ir StageInfo<L>,
        block: Block,
    ) -> Result<Continuation<Self::Value, Self::Ext>, Self::Error>
    where
        Self::StageInfo: HasStageInfo<L>,
        L: crate::Interpretable<'ir, Self, L>;
}
