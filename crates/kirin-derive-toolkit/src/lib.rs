//! A metaprogramming toolkit for building Kirin dialect derive macros.
//!
//! # Architecture
//!
//! The toolkit follows a four-stage pipeline:
//!
//! ```text
//! syn::DeriveInput ──► Input<L> ──► Scan ──► Emit ──► TokenStream
//!      (Rust AST)     (IR parse)  (collect)  (codegen)  (output)
//! ```
//!
//! ## Layers
//!
//! | Layer | Modules | Purpose |
//! |-------|---------|---------|
//! | **IR** | [`ir`], [`ir::fields`] | Parsed representation of derive input — types, fields, attributes |
//! | **Visitors** | [`scan`], [`emit`] | Two-pass visitor pattern: scan collects metadata, emit generates code |
//! | **Generators** | [`generators`] | Pre-built generators for common derives (builder, field iterators, properties) |
//! | **Tokens** | [`tokens`], [`codegen`] | Typed code-block builders (`TraitImpl`, `MatchExpr`, etc.) and utilities |
//! | **Support** | [`context`], [`mod@derive`], [`stage`], [`misc`] | Pre-computed state, metadata extraction, stage parsing |
//!
//! ## Quick Start
//!
//! Most derives follow this pattern:
//!
//! 1. Parse: `Input::<StandardLayout>::from_derive_input(&ast)?`
//! 2. Implement [`Scan`] to collect per-statement metadata
//! 3. Implement [`Emit`] to generate code for each statement
//! 4. Or compose pre-built [`generators`] via `input.generate().with(gen).emit()?`
//!
//! ## Layout Extensibility
//!
//! [`StandardLayout`] works for most derives. If your derive needs custom attributes
//! on statements or fields (e.g., `#[callable]`), define a custom [`Layout`] impl.
//! See [`ir::Layout`] for details.
//!
//! [`Scan`]: scan::Scan
//! [`Emit`]: emit::Emit
//! [`Layout`]: ir::Layout
//! [`StandardLayout`]: ir::StandardLayout

pub mod codegen;
pub mod context;
pub mod derive;
pub mod emit;
pub mod generator;
pub mod generators;
pub mod ir;
pub mod misc;
pub mod scan;
pub mod stage;
pub mod test_util;
pub mod tokens;

pub mod prelude {
    pub use crate::codegen::{
        self, ConstructorBuilder, FieldBindings, GenericsBuilder, combine_where_clauses,
        deduplicate_types,
    };
    pub use crate::derive::{self, InputMeta, PathBuilder};
    pub use crate::emit::{self, Emit};
    pub use crate::ir::fields::{FieldCategory, FieldData, FieldInfo};
    pub use crate::ir::{self, Layout, StandardLayout};
    pub use crate::scan::{self, Scan};
    pub use crate::tokens;
    pub use darling;
    pub use proc_macro2;
}
