use kirin::prelude::*;
use kirin_interpreter::InterpDispatch;

use crate::language::{HighLevel, LowLevel};

#[derive(Debug, StageMeta, ParseDispatch, InterpDispatch, RenderDispatch)]
pub enum Stage {
    #[stage(name = "source")]
    Source(StageInfo<HighLevel>),
    #[stage(name = "lowered")]
    Lowered(StageInfo<LowLevel>),
}
