use kirin_ir::{CompileStage, Id, Pipeline, StageDispatchMiss, StageMeta, SupportsStageDispatch};

use crate::InterpreterError;

/// Cached per-stage dispatch results.
///
/// Pre-computes one entry per stage at construction time. Runtime lookups are
/// O(1) by stage index. Custom interpreter developers can use this with their
/// own entry types.
pub struct DispatchCache<Entry> {
    by_stage: Vec<Option<Entry>>,
}

impl<Entry> DispatchCache<Entry> {
    /// Create an empty cache (no stages).
    pub fn empty() -> Self {
        Self {
            by_stage: Vec::new(),
        }
    }

    /// Returns `true` when the cache has no entries.
    pub fn is_empty(&self) -> bool {
        self.by_stage.is_empty()
    }

    /// Build a dispatch cache by resolving each stage in the pipeline.
    pub fn build<S, E>(
        pipeline: &Pipeline<S>,
        mut resolve: impl FnMut(&Pipeline<S>, CompileStage) -> Result<Entry, E>,
    ) -> Self
    where
        S: StageMeta,
    {
        let mut by_stage = Vec::with_capacity(pipeline.stages().len());
        for stage in pipeline.stages() {
            let entry = stage
                .stage_id()
                .and_then(|stage_id| resolve(pipeline, stage_id).ok());
            by_stage.push(entry);
        }
        Self { by_stage }
    }

    /// Look up a cached entry by stage index.
    pub fn get(&self, stage: CompileStage) -> Option<&Entry> {
        let idx = Id::from(stage).raw();
        self.by_stage.get(idx).and_then(|e| e.as_ref())
    }

    /// Look up a cached entry by stage index (mutable).
    pub fn get_mut(&mut self, stage: CompileStage) -> Option<&mut Entry> {
        let idx = Id::from(stage).raw();
        self.by_stage.get_mut(idx).and_then(|e| e.as_mut())
    }
}

/// Convert a stage-dispatch miss into the framework error model.
pub fn map_dispatch_miss<E: From<InterpreterError>>(
    stage_id: CompileStage,
    miss: StageDispatchMiss,
) -> E {
    match miss {
        StageDispatchMiss::MissingStage => InterpreterError::StageResolution {
            stage: stage_id,
            kind: crate::StageResolutionError::MissingStage,
        },
        StageDispatchMiss::MissingDialect => InterpreterError::StageResolution {
            stage: stage_id,
            kind: crate::StageResolutionError::MissingDialect,
        },
    }
    .into()
}

/// Dispatch a runtime action against `stage_id` using `pipeline`.
pub fn dispatch_in_pipeline<S, A, R, E>(
    pipeline: &Pipeline<S>,
    stage_id: CompileStage,
    action: &mut A,
) -> Result<R, E>
where
    S: StageMeta + SupportsStageDispatch<A, R, E>,
    E: From<InterpreterError>,
{
    pipeline.dispatch_stage_or_else(stage_id, action, |miss| map_dispatch_miss(stage_id, miss))
}
