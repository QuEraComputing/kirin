use std::fmt;
use std::marker::PhantomData;

use kirin_ir::{
    Block, CompileStage, Dialect, GetInfo, HasStageInfo, Pipeline, ResultValue, SSAValue,
    StageDispatchMiss, StageInfo, StageMeta, SupportsStageDispatch,
};

use crate::InterpreterError;
use crate::stage::{InStage, WithStage};

/// Minimal state contract for interpreter implementations.
///
/// Provides SSA value read/write only. The associated `Ext` type
/// determines which extra continuation variants are available — concrete
/// interpreters use [`crate::ConcreteExt`] while abstract interpreters
/// use [`std::convert::Infallible`].
///
/// Call semantics are inherent on each interpreter type
/// (e.g. [`crate::StackInterpreter::call`],
/// [`crate::AbstractInterpreter::analyze`]).
pub trait Interpreter<'ir>: Sized + 'ir {
    /// The value type manipulated by this interpreter.
    ///
    /// Values should be cheap to clone — typically pointer-sized handles,
    /// small enums, or wrappers around `Arc`/`Rc` for heavier data.
    type Value: Clone;
    type Error;
    type Ext: fmt::Debug;
    type StageInfo: StageMeta;

    /// Returns a cloned copy of the bound value.
    fn read(&self, value: SSAValue) -> Result<Self::Value, Self::Error>;

    /// Bind a result to a value.
    fn write(&mut self, result: ResultValue, value: Self::Value) -> Result<(), Self::Error>;

    /// Bind an SSA value directly (e.g. block arguments).
    fn write_ssa(&mut self, ssa: SSAValue, value: Self::Value) -> Result<(), Self::Error>;

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
        let stage = self
            .pipeline()
            .stage(stage_id)
            .ok_or_else(|| InterpreterError::MissingStage { stage: stage_id })?;
        <Self::StageInfo as HasStageInfo<L>>::try_stage_info(stage).ok_or_else(|| {
            InterpreterError::TypedStageMismatch {
                frame_stage: stage_id,
            }
            .into()
        })
    }

    /// Convert a stage-dispatch miss into the framework error model.
    fn map_dispatch_miss(stage_id: CompileStage, miss: StageDispatchMiss) -> Self::Error
    where
        Self::Error: From<InterpreterError>,
    {
        match miss {
            StageDispatchMiss::MissingStage => InterpreterError::MissingStage { stage: stage_id },
            StageDispatchMiss::MissingDialect => {
                InterpreterError::MissingStageDialect { stage: stage_id }
            }
        }
        .into()
    }

    /// Dispatch a runtime action against `stage_id` using `pipeline`, mapping
    /// dispatch misses to [`InterpreterError`] variants.
    fn dispatch_in_pipeline<A, R>(
        pipeline: &'ir Pipeline<Self::StageInfo>,
        stage_id: CompileStage,
        action: &mut A,
    ) -> Result<R, Self::Error>
    where
        Self::StageInfo: SupportsStageDispatch<A, R, Self::Error>,
        Self::Error: From<InterpreterError>,
    {
        pipeline.dispatch_stage_or_else(stage_id, action, |miss| {
            Self::map_dispatch_miss(stage_id, miss)
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

    /// Resolve typed-stage APIs from the current active stage.
    fn in_stage<L>(&mut self) -> InStage<'_, Self, L> {
        InStage {
            interp: self,
            marker: PhantomData,
        }
    }

    /// Bind APIs to an explicit stage reference.
    fn with_stage<L: Dialect>(&mut self, stage: &'ir StageInfo<L>) -> WithStage<'_, 'ir, Self, L> {
        WithStage {
            interp: self,
            stage,
        }
    }
}
