use crate::context::DeriveContext;
use crate::ir::{Input, Layout};
use crate::misc::debug_dump;
use proc_macro2::TokenStream;

use super::Template;

/// Fluent builder for composing templates over an [`Input`].
///
/// Created via [`Input::compose()`]. Chain templates with `.add()`,
/// then call `.build()` to run them all.
pub struct TemplateBuilder<'ir, L: Layout> {
    ctx: DeriveContext<'ir, L>,
    templates: Vec<Box<dyn Template<L>>>,
}

impl<L: Layout> Input<L> {
    /// Start building a template pipeline over this input.
    pub fn compose(&self) -> TemplateBuilder<'_, L> {
        TemplateBuilder {
            ctx: DeriveContext::new(self),
            templates: Vec::new(),
        }
    }
}

impl<'ir, L: Layout> TemplateBuilder<'ir, L> {
    /// Append a template to the pipeline.
    pub fn add(mut self, t: impl Template<L> + 'static) -> Self {
        self.templates.push(Box::new(t));
        self
    }

    /// Access the DeriveContext for inspection.
    pub fn context(&self) -> &DeriveContext<'ir, L> {
        &self.ctx
    }

    /// Run all templates and combine their output into a single `TokenStream`.
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
