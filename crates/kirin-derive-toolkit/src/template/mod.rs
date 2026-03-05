//! Template-based code generation system.
//!
//! Templates are composable building blocks for derive macro code generation.
//! Each template generates one or more `TokenStream` fragments from a
//! [`DeriveContext`](crate::context::DeriveContext).
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

pub mod inherent_impl;
pub mod method_pattern;
pub mod trait_impl;
pub mod type_def;

mod builder_template;
mod field_iter_set;

pub use builder_template::BuilderTemplate;
pub use field_iter_set::FieldIterTemplateSet;
pub use inherent_impl::InherentImplTemplate;
pub use trait_impl::{BoolPropertyConfig, FieldIterConfig, MarkerTemplate, TraitImplTemplate};
pub use type_def::TypeDefTemplate;

use crate::context::DeriveContext;
use crate::generator::debug_dump;
use crate::ir::{Input, Layout};
use proc_macro2::TokenStream;

/// A composable code generation building block.
///
/// Each template receives a pre-computed [`DeriveContext`] and produces
/// one or more `TokenStream` fragments.
pub trait Template<L: Layout> {
    fn emit(&self, ctx: &DeriveContext<'_, L>) -> darling::Result<Vec<TokenStream>>;
}

/// Groups multiple templates into one.
pub struct CompositeTemplate<L: Layout> {
    templates: Vec<Box<dyn Template<L>>>,
}

impl<L: Layout> CompositeTemplate<L> {
    pub fn new() -> Self {
        Self {
            templates: Vec::new(),
        }
    }

    pub fn add(mut self, t: impl Template<L> + 'static) -> Self {
        self.templates.push(Box::new(t));
        self
    }
}

impl<L: Layout> Default for CompositeTemplate<L> {
    fn default() -> Self {
        Self::new()
    }
}

impl<L: Layout> Template<L> for CompositeTemplate<L> {
    fn emit(&self, ctx: &DeriveContext<'_, L>) -> darling::Result<Vec<TokenStream>> {
        let mut results = Vec::new();
        let mut errors = darling::Error::accumulator();
        for t in &self.templates {
            errors.handle_in(|| {
                results.extend(t.emit(ctx)?);
                Ok(())
            });
        }
        errors.finish()?;
        Ok(results)
    }
}

/// Fluent builder for composing templates over an [`Input`].
///
/// Created via [`Input::compose()`]. Chain templates with `.add()`,
/// then call `.build()` to run them all.
pub struct TemplateBuilder<'ir, L: Layout> {
    ctx: DeriveContext<'ir, L>,
    templates: Vec<Box<dyn Template<L>>>,
}

impl<L: Layout> Input<L> {
    pub fn compose(&self) -> TemplateBuilder<'_, L> {
        TemplateBuilder {
            ctx: DeriveContext::new(self),
            templates: Vec::new(),
        }
    }
}

impl<'ir, L: Layout> TemplateBuilder<'ir, L> {
    pub fn add(mut self, t: impl Template<L> + 'static) -> Self {
        self.templates.push(Box::new(t));
        self
    }

    /// Access the DeriveContext for inspection.
    pub fn context(&self) -> &DeriveContext<'ir, L> {
        &self.ctx
    }

    pub fn build(self) -> darling::Result<TokenStream> {
        let mut combined = TokenStream::new();
        let mut errors = darling::Error::accumulator();

        for t in &self.templates {
            errors.handle_in(|| {
                let fragments = t.emit(&self.ctx)?;
                for fragment in fragments {
                    combined.extend(fragment);
                }
                Ok(())
            });
        }

        errors.finish()?;
        debug_dump(&combined);
        Ok(combined)
    }
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
