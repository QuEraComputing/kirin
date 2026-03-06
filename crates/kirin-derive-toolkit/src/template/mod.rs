//! Template-based code generation system.
//!
//! Templates are composable building blocks for derive macro code generation.
//! Each template generates one or more `TokenStream` fragments from a
//! [`DeriveContext`].
//!
//! # Layers
//!
//! | Layer | Usage |
//! |-------|-------|
//! | **Declarative** | Factory methods: `TraitImplTemplate::bool_property(...)` |
//! | **Composition** | Template + MethodPattern: `.method("interpret", DelegateToWrapper::new(...))` |
//! | **Custom** | Closure-based: `.method("cfg", Custom::new(\|ctx, stmt\| { ... }))` |
//!
//! # Example
//!
//! ```ignore
//! let ir = Input::<StandardLayout>::from_derive_input(&ast)?;
//! ir.compose()
//!     .add(TraitImplTemplate::bool_property(IS_PURE_CONFIG, "::kirin::ir"))
//!     .add(TraitImplTemplate::marker(&trait_path, &ir_type))
//!     .build()
//! ```

mod builder;
mod composite;

pub mod inherent_impl;
pub mod method_pattern;
pub mod trait_impl;
pub mod type_def;

mod builder_template;
mod field_iter_set;

pub use builder::TemplateBuilder;
pub use builder_template::BuilderTemplate;
pub use composite::CompositeTemplate;
pub use field_iter_set::FieldIterTemplateSet;
pub use inherent_impl::InherentImplTemplate;
pub use trait_impl::{BoolPropertyConfig, FieldIterConfig, MarkerTemplate, TraitImplTemplate};
pub use type_def::TypeDefTemplate;

use crate::context::DeriveContext;
use crate::ir::Layout;
use proc_macro2::TokenStream;

/// A composable code generation building block.
///
/// Each template receives a pre-computed [`DeriveContext`] and produces
/// one or more `TokenStream` fragments.
pub trait Template<L: Layout> {
    /// Generate token stream fragments from the given derive context.
    fn emit(&self, ctx: &DeriveContext<'_, L>) -> darling::Result<Vec<TokenStream>>;
}

/// Blanket impl: closures can be used as templates.
impl<L, F> Template<L> for F
where
    L: Layout,
    F: Fn(&DeriveContext<'_, L>) -> darling::Result<Vec<TokenStream>>,
{
    fn emit(&self, ctx: &DeriveContext<'_, L>) -> darling::Result<Vec<TokenStream>> {
        self(ctx)
    }
}
