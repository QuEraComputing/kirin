use kirin_ir::{CompileStage, Dialect, StageInfo};

/// Extract the stage ID from a `StageInfo`, panicking if it is not attached
/// to a pipeline stage.
pub(crate) fn expect_stage_id<L: Dialect>(stage: &StageInfo<L>) -> CompileStage {
    stage
        .stage_id()
        .expect("stage info must be attached to a pipeline stage")
}

/// Unified typed-stage API builder.
///
/// Created by [`Interpreter::in_stage`] (eagerly resolves the active stage)
/// or [`Interpreter::with_stage`] (takes an explicit stage reference).
pub struct Staged<'a, 'ir, I, L: Dialect> {
    pub(crate) interp: &'a mut I,
    pub(crate) stage: &'ir StageInfo<L>,
}

impl<I, L: Dialect> std::fmt::Debug for Staged<'_, '_, I, L>
where
    StageInfo<L>: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Staged")
            .field("stage", &self.stage)
            .finish_non_exhaustive()
    }
}
