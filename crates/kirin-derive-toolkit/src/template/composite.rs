use crate::context::DeriveContext;
use crate::ir::Layout;
use proc_macro2::TokenStream;

use super::Template;

/// Groups multiple templates into one.
pub struct CompositeTemplate<L: Layout> {
    templates: Vec<Box<dyn Template<L>>>,
}

impl<L: Layout> CompositeTemplate<L> {
    /// Create an empty composite template.
    pub fn new() -> Self {
        Self {
            templates: Vec::new(),
        }
    }

    /// Append a template to the group.
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
