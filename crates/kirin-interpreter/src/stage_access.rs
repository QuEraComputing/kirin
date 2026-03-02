use kirin_ir::{CompileStage, Dialect, HasStageInfo, Pipeline, StageInfo, StageMeta};

use crate::InterpreterError;
use crate::stage::Staged;

/// Stage resolution and typed-stage access.
///
/// Provides the pipeline reference, active stage tracking, and convenience
/// methods for resolving dialect-specific [`StageInfo`] views.
///
/// This trait is a supertrait of [`crate::Interpreter`]; dialect authors
/// rarely need to use it directly since all methods are available through
/// the interpreter.
pub trait StageAccess<'ir>: Sized + 'ir {
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
    ) -> Result<&'ir StageInfo<L>, InterpreterError>
    where
        Self::StageInfo: HasStageInfo<L>,
        L: Dialect,
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
        })
    }

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

    /// Convenience: resolve typed [`StageInfo`] for the current active stage.
    ///
    /// Equivalent to `self.resolve_stage_info(self.active_stage())`.
    fn resolve_stage<L>(&self) -> Result<&'ir StageInfo<L>, InterpreterError>
    where
        Self::StageInfo: HasStageInfo<L>,
        L: Dialect,
    {
        self.resolve_stage_info(self.active_stage())
    }

    /// Convenience: resolve typed [`StageInfo`] for the current active stage.
    ///
    /// Equivalent to `self.resolve_stage_info(self.active_stage())`.
    fn resolve_stage<L>(&self) -> Result<&'ir StageInfo<L>, InterpreterError>
    where
        Self::StageInfo: HasStageInfo<L>,
        L: Dialect,
    {
        self.resolve_stage_info(self.active_stage())
    }

    /// Bind APIs to an explicit stage reference.
    fn with_stage<L: Dialect>(&mut self, stage: &'ir StageInfo<L>) -> Staged<'_, 'ir, Self, L> {
        Staged {
            interp: self,
            stage,
        }
    }
}
