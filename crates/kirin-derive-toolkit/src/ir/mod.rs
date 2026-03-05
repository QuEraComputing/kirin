//! Parsed IR representation of derive macro input.
//!
//! This module provides a three-level hierarchy that mirrors how Kirin dialect
//! types are structured:
//!
//! ```text
//! Input<L>          -- top-level struct/enum being derived
//!   └─ Statement<L> -- each variant (enum) or the single body (struct)
//!       └─ FieldInfo<L> -- each field, classified by category
//! ```
//!
//! The [`Layout`] trait parameterizes the IR so different derives can attach
//! custom attributes at each level. [`StandardLayout`] uses `()` for all extras
//! and is the right choice for most derives.
//!
//! # Parsing
//!
//! ```ignore
//! let ir = Input::<StandardLayout>::from_derive_input(&ast)?;
//! ```

mod attrs;
pub mod fields;
mod input;
mod layout;
mod statement;

pub use attrs::{BuilderOptions, DefaultValue, GlobalOptions, KirinFieldOptions, StatementOptions};
pub use input::{Data, DataEnum, DataStruct, Input, VariantRef};
pub use layout::{Layout, StandardLayout};
pub use statement::Statement;
