use crate::context::DeriveContext;
use crate::ir::Layout;
use proc_macro2::TokenStream;

use super::Template;

/// Template that generates `impl Type { methods }` blocks.
///
/// Uses a closure to compute the impl from the DeriveContext.
pub struct InherentImplTemplate<L: Layout> {
    generate: Box<dyn Fn(&DeriveContext<'_, L>) -> darling::Result<Vec<TokenStream>>>,
}

impl<L: Layout> InherentImplTemplate<L> {
    pub fn new(
        f: impl Fn(&DeriveContext<'_, L>) -> darling::Result<Vec<TokenStream>> + 'static,
    ) -> Self {
        Self {
            generate: Box::new(f),
        }
    }
}

impl<L: Layout> Template<L> for InherentImplTemplate<L> {
    fn emit(&self, ctx: &DeriveContext<'_, L>) -> darling::Result<Vec<TokenStream>> {
        (self.generate)(ctx)
    }
}
