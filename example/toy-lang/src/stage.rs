use kirin::prelude::*;

use crate::language::{HighLevel, LowLevel};

#[derive(Debug, StageMeta, ParseDispatch, RenderStage)]
pub enum Stage {
    #[stage(name = "source")]
    Source(StageInfo<HighLevel>),
    #[stage(name = "lowered")]
    Lowered(StageInfo<LowLevel>),
}
