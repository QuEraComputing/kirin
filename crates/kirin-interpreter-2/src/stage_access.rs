use kirin_ir::{CompileStage, Dialect, HasStageInfo, Pipeline, StageInfo, StageMeta};

use crate::{InterpreterError, StageResolutionError};

/// Minimal typed stage access for stage-local shells.
pub trait StageAccess<'ir>: Sized + 'ir {
    type StageInfo: StageMeta;

    fn pipeline(&self) -> &'ir Pipeline<Self::StageInfo>;

    fn active_stage(&self) -> CompileStage;

    fn active_stage_info<L>(&self) -> &'ir StageInfo<L>
    where
        Self::StageInfo: HasStageInfo<L>,
        L: Dialect,
    {
        self.pipeline()
            .stage(self.active_stage())
            .and_then(|stage| stage.try_stage_info())
            .expect("active stage does not contain StageInfo for this dialect")
    }

    fn resolve_stage_id<L: Dialect>(&self, stage: &StageInfo<L>) -> CompileStage {
        stage.stage_id().unwrap_or_else(|| self.active_stage())
    }

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
                    kind: StageResolutionError::MissingStage,
                })?;

        <Self::StageInfo as HasStageInfo<L>>::try_stage_info(stage).ok_or_else(|| {
            InterpreterError::StageResolution {
                stage: stage_id,
                kind: StageResolutionError::TypeMismatch,
            }
        })
    }

    fn resolve_stage<L>(&self) -> Result<&'ir StageInfo<L>, InterpreterError>
    where
        Self::StageInfo: HasStageInfo<L>,
        L: Dialect,
    {
        self.resolve_stage_info(self.active_stage())
    }
}
