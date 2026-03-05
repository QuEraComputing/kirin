//! A metaprogramming toolkit for building Kirin dialect derive macros.
//!
//! # Architecture
//!
//! The toolkit is built around a **template system** where composable templates
//! handle code structure and method patterns handle per-variant logic:
//!
//! ```text
//! syn::DeriveInput ──► Input<L> ──► DeriveContext ──► Templates ──► TokenStream
//!      (Rust AST)     (IR parse)   (pre-computed)    (codegen)      (output)
//! ```
//!
//! ## Layers
//!
//! | Layer | Modules | Purpose |
//! |-------|---------|---------|
//! | **IR** | [`ir`], [`ir::fields`] | Parsed representation of derive input — types, fields, attributes |
//! | **Templates** | [`template`] | Composable code generation: `TraitImplTemplate`, `MethodPattern`, factory methods |
//! | **Tokens** | [`tokens`], [`codegen`] | Typed code-block builders (`TraitImpl`, `MatchExpr`, etc.) and utilities |
//! | **Support** | [`context`], [`mod@derive`], [`stage`], [`misc`] | Pre-computed state, metadata extraction, stage parsing |
//! | **Legacy** | [`scan`], [`emit`], [`generators`] | Two-pass visitor pattern (used by kirin-derive-chumsky) |
//!
//! ## Quick Start
//!
//! Most derives use the template system:
//!
//! 1. Parse: `Input::<StandardLayout>::from_derive_input(&ast)?`
//! 2. Compose templates: `input.compose().add(template1).add(template2).build()?`
//! 3. Or use factory methods: `TraitImplTemplate::bool_property(config, crate_path)`
//!
//! For custom logic, use closures as templates or `Custom` method patterns.
//!
//! ## Layout Extensibility
//!
//! [`StandardLayout`] works for most derives. If your derive needs custom attributes
//! on statements or fields (e.g., `#[callable]`), define a custom [`Layout`] impl.
//! See [`ir::Layout`] for details.
//!
//! [`Layout`]: ir::Layout
//! [`StandardLayout`]: ir::StandardLayout

pub mod codegen;
pub mod context;
pub mod derive;
pub mod emit;
pub mod generator;
pub mod generators;
pub mod hygiene;
pub mod ir;
pub mod misc;
pub mod scan;
pub mod stage;
pub mod template;
pub mod test_util;
pub mod tokens;

pub mod prelude {
    pub use crate::codegen::{
        self, ConstructorBuilder, FieldBindings, GenericsBuilder, combine_where_clauses,
        deduplicate_types,
    };
    pub use crate::context::{DeriveContext, StatementContext};
    pub use crate::derive::{self, InputMeta, PathBuilder};
    pub use crate::ir::fields::{FieldCategory, FieldData, FieldInfo};
    pub use crate::ir::{self, Layout, StandardLayout};
    pub use crate::template::{
        self, BuilderTemplate, CompositeTemplate, FieldIterTemplateSet, MarkerTemplate, Template,
        TemplateBuilder, TraitImplTemplate,
        method_pattern::{self, AssocTypeSpec, Custom, MethodPattern, MethodSpec},
        trait_impl::{BoolPropertyConfig, FieldIterConfig},
    };
    pub use crate::tokens;
    pub use darling;
    pub use proc_macro2;

    // Legacy re-exports for kirin-derive-chumsky compatibility
    pub use crate::emit::{self, Emit};
    pub use crate::scan::{self, Scan};
}
