use kirin_ir::{Dialect, StageInfo};

use crate::StageAccess;

/// Typed access to the active dialect stage for stage-local shells.
pub trait TypedStage<'ir>: StageAccess<'ir> {
    type Dialect: Dialect + 'ir;

    fn stage_info(&self) -> &'ir StageInfo<Self::Dialect>;
}
