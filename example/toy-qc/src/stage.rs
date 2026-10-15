use kirin::prelude::*;

use crate::circuit::Circuit;
use crate::zx::ZX;

#[derive(Debug, StageMeta, ParseDispatch, RenderDispatch)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Stage {
    #[stage(name = "circuit")]
    Circuit(StageInfo<Circuit>),
    #[stage(name = "zx")]
    ZX(StageInfo<ZX>),
}
