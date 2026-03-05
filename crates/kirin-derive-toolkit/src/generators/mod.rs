//! Pre-built generators for common Kirin derive patterns.
//!
//! | Generator | Emits |
//! |-----------|-------|
//! | [`builder::DeriveBuilder`] | Constructor `new()` functions |
//! | [`field::DeriveFieldIter`] | Field iterator trait impls (`HasArguments`, `HasResults`, etc.) |
//! | [`property::DeriveProperty`] | Property trait impls (`IsTerminator`, `IsPure`, etc.) |
//! | [`marker::derive_marker`] | Marker trait `Type` associated type |
//! | [`stage_info::generate`] | `StageMeta` and `HasStageInfo` impls for stage enums |
//!
//! These can be used standalone or composed via [`GenerateBuilder`](crate::generator::GenerateBuilder).

mod common;

pub mod builder;
pub mod field;
pub mod marker;
pub mod property;
pub mod stage_info;
