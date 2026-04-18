use kirin_ir::{
    Block, CompileStage, Dialect, GetInfo, HasStageInfo, Pipeline, SpecializedFunction, StageInfo,
    StageMeta, Symbol,
};

use crate::error::InterpreterError;

/// Shared pipeline-navigation handle.
pub struct PipelineHandle<'ir, S: StageMeta> {
    pub pipeline: &'ir Pipeline<S>,
    pub stage_id: CompileStage,
}

impl<'ir, S: StageMeta> PipelineHandle<'ir, S> {
    pub fn new(pipeline: &'ir Pipeline<S>, stage_id: CompileStage) -> Self {
        Self { pipeline, stage_id }
    }

    /// Look up the `StageInfo<L>` for a given stage ID.
    pub fn stage_info_for<L: Dialect>(&self, stage_id: CompileStage) -> Option<&'ir StageInfo<L>>
    where
        S: HasStageInfo<L>,
    {
        self.pipeline.stage(stage_id)?.try_stage_info()
    }

    /// Resolve a function symbol to a `SpecializedFunction` using dialect `L`.
    pub fn resolve_function_for<L: Dialect>(
        &self,
        target: Symbol,
        stage_id: CompileStage,
    ) -> Result<SpecializedFunction, InterpreterError>
    where
        S: HasStageInfo<L>,
    {
        let stage_container = self
            .pipeline
            .stage(stage_id)
            .ok_or(InterpreterError::MissingEntry)?;
        let stage_info: &StageInfo<L> = stage_container
            .try_stage_info()
            .ok_or(InterpreterError::MissingEntry)?;
        let function = self
            .pipeline
            .resolve_function(stage_info, target)
            .ok_or(InterpreterError::MissingEntry)?;
        let staged_function = self
            .pipeline
            .function_info(function)
            .and_then(|info| info.staged_function(stage_id))
            .ok_or(InterpreterError::MissingEntry)?;
        staged_function
            .get_info(stage_info)
            .ok_or(InterpreterError::MissingEntry)?
            .unique_live_specialization()
            .map_err(|_| InterpreterError::UnhandledEffect("ambiguous specialization".into()))
    }

    /// Find the entry block of a specialized function for dialect `L`.
    pub fn entry_block_of<L: Dialect>(
        &self,
        func: SpecializedFunction,
        stage_id: CompileStage,
    ) -> Result<Block, InterpreterError>
    where
        S: HasStageInfo<L>,
    {
        let stage: &StageInfo<L> = self
            .pipeline
            .stage(stage_id)
            .and_then(|s| s.try_stage_info())
            .ok_or(InterpreterError::MissingEntry)?;
        let spec_info = func.get_info(stage).ok_or(InterpreterError::MissingEntry)?;
        let body_stmt = *spec_info.body();
        let definition = body_stmt.definition(stage).clone();
        definition
            .regions()
            .next()
            .ok_or(InterpreterError::MissingEntry)
            .and_then(|region| {
                region
                    .blocks(stage)
                    .next()
                    .ok_or(InterpreterError::MissingEntry)
            })
    }
}
