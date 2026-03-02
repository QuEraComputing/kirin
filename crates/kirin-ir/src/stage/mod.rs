mod action;
mod dispatch;
mod error;
mod helpers;
mod meta;
mod pipeline_impl;

pub use action::{StageAction, StageActionMut};
pub use dispatch::{
    StageDispatch, StageDispatchMut, SupportsStageDispatch, SupportsStageDispatchMut,
};
pub use error::{StageDispatchMiss, StageDispatchRequiredError};
pub use meta::{HasStageInfo, StageMeta};

#[cfg(test)]
mod tests;
