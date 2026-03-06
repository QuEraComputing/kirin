//! Field classification algebra for IR statements.
//!
//! Every field in a Kirin statement is automatically classified by its Rust type:
//!
//! | Rust Type | Category | Meaning |
//! |-----------|----------|---------|
//! | `SSAValue` / `SSAValue<T>` | [`Argument`](FieldCategory::Argument) | SSA input value |
//! | `ResultValue` / `ResultValue<T>` | [`Result`](FieldCategory::Result) | SSA output value |
//! | `Block` | [`Block`](FieldCategory::Block) | Basic block reference |
//! | `Successor` | [`Successor`](FieldCategory::Successor) | Control-flow successor |
//! | `Region` / `Region<T>` | [`Region`](FieldCategory::Region) | Nested region |
//! | `Symbol` | [`Symbol`](FieldCategory::Symbol) | Symbol reference |
//! | anything else | [`Value`](FieldCategory::Value) | Plain Rust value |
//!
//! Each field also tracks its [`Collection`] wrapping: `Single`, `Vec`, or `Option`.

mod collection;
mod data;
mod index;
mod info;
mod wrapper;

pub use collection::Collection;
pub use data::{FieldCategory, FieldData};
pub use index::FieldIndex;
pub use info::FieldInfo;
pub use wrapper::Wrapper;
