use std::marker::PhantomData;

use kirin_ir::{Dialect, HasStageInfo, StageInfo};

use crate::{Interpreter, InterpreterError};

/// Typed-stage API builder resolved from the interpreter's active stage.
pub struct InStage<'a, I, L> {
    pub(crate) interp: &'a mut I,
    pub(crate) marker: PhantomData<L>,
}

/// API builder for an explicitly resolved [`StageInfo`].
pub struct WithStage<'a, 'ir, I, L: Dialect> {
    pub(crate) interp: &'a mut I,
    pub(crate) stage: &'ir StageInfo<L>,
}

impl<'a, 'ir, I, L> InStage<'a, I, L>
where
    I: Interpreter<'ir>,
    I::StageInfo: HasStageInfo<L>,
    I::Error: From<InterpreterError>,
    L: Dialect,
{
    pub(crate) fn resolve_active_stage_info(&self) -> Result<&'ir StageInfo<L>, I::Error> {
        let stage_id = self.interp.active_stage();
        self.interp.resolve_stage_info::<L>(stage_id)
    }
}
