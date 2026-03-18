mod action;
pub(crate) mod arenas;
mod dispatch;
mod error;
mod helpers;
pub(crate) mod info;
mod meta;
mod pipeline_impl;

pub use action::{StageAction, StageActionMut};
pub use arenas::Arenas as StageArenas;
pub use dispatch::{
    StageDispatch, StageDispatchMut, SupportsStageDispatch, SupportsStageDispatchMut,
};
pub use error::{StageDispatchMiss, StageDispatchRequiredError};
pub use info::StageInfo;
pub use meta::{HasStageInfo, StageMeta};

#[cfg(test)]
mod tests;
