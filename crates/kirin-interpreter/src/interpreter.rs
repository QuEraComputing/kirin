use std::fmt;

use kirin_ir::{
    Block, CompileStage, Dialect, GetInfo, HasStageInfo, Pipeline, SSAValue, StageInfo, StageMeta,
};

use crate::Continuation;
use crate::InterpreterError;
use crate::ValueStore;
use crate::stage::Staged;

/// Minimal state contract for interpreter implementations.
///
/// Requires [`ValueStore`] for SSA value read/write. The associated `Ext` type
/// determines which extra continuation variants are available — concrete
/// interpreters use [`crate::ConcreteExt`] while abstract interpreters
/// use [`std::convert::Infallible`].
///
/// Call semantics are inherent on each interpreter type
/// (e.g. [`crate::StackInterpreter::call`],
/// [`crate::AbstractInterpreter::analyze`]).
pub trait Interpreter<'ir>: ValueStore + Sized + 'ir {
    type Ext: fmt::Debug;
    type StageInfo: StageMeta;

    /// Reference to the IR pipeline.
    fn pipeline(&self) -> &'ir Pipeline<Self::StageInfo>;

    /// The currently active compilation stage.
    fn active_stage(&self) -> CompileStage;

    /// Resolve the [`StageInfo`] for dialect `L` from the active stage.
    ///
    /// # Panics
    ///
    /// Panics if the active stage does not contain a `StageInfo<L>`.
    fn active_stage_info<L>(&self) -> &'ir StageInfo<L>
    where
        Self::StageInfo: HasStageInfo<L>,
        L: Dialect,
    {
        self.pipeline()
            .stage(self.active_stage())
            .and_then(|s| s.try_stage_info())
            .expect("active stage does not contain StageInfo for this dialect")
    }

    /// Returns the stage ID from `stage`, falling back to the active stage
    /// if the stage info is not attached to a pipeline stage.
    fn resolve_stage_id<L: Dialect>(&self, stage: &StageInfo<L>) -> CompileStage {
        stage.stage_id().unwrap_or_else(|| self.active_stage())
    }

    /// Resolve a stage-specific dialect view for `stage_id` with explicit
    /// errors instead of panicking.
    fn resolve_stage_info<L>(
        &self,
        stage_id: CompileStage,
    ) -> Result<&'ir StageInfo<L>, Self::Error>
    where
        Self::StageInfo: HasStageInfo<L>,
        L: Dialect,
        Self::Error: From<InterpreterError>,
    {
        let stage =
            self.pipeline()
                .stage(stage_id)
                .ok_or_else(|| InterpreterError::StageResolution {
                    stage: stage_id,
                    kind: crate::StageResolutionError::MissingStage,
                })?;
        <Self::StageInfo as HasStageInfo<L>>::try_stage_info(stage).ok_or_else(|| {
            InterpreterError::StageResolution {
                stage: stage_id,
                kind: crate::StageResolutionError::TypeMismatch,
            }
            .into()
        })
    }

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

    /// Resolve typed-stage APIs from the current active stage.
    fn in_stage<L>(&mut self) -> Staged<'_, 'ir, Self, L>
    where
        Self::StageInfo: HasStageInfo<L>,
        L: Dialect,
    {
        let stage = self.active_stage_info::<L>();
        Staged {
            interp: self,
            stage,
        }
    }

    /// Bind APIs to an explicit stage reference.
    fn with_stage<L: Dialect>(&mut self, stage: &'ir StageInfo<L>) -> Staged<'_, 'ir, Self, L> {
        Staged {
            interp: self,
            stage,
        }
    }
}
