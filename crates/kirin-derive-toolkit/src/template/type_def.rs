use crate::context::DeriveContext;
use crate::ir::Layout;
use proc_macro2::TokenStream;

use super::Template;

/// Template that generates struct/enum definitions (e.g., iterator newtypes).
///
/// Uses a closure to compute the definition from the DeriveContext.
pub struct TypeDefTemplate<L: Layout> {
    generate: Box<dyn Fn(&DeriveContext<'_, L>) -> darling::Result<Vec<TokenStream>>>,
}

impl<L: Layout> TypeDefTemplate<L> {
    pub fn new(
        f: impl Fn(&DeriveContext<'_, L>) -> darling::Result<Vec<TokenStream>> + 'static,
    ) -> Self {
        Self {
            generate: Box::new(f),
        }
    }
}

impl<L: Layout> Template<L> for TypeDefTemplate<L> {
    fn emit(&self, ctx: &DeriveContext<'_, L>) -> darling::Result<Vec<TokenStream>> {
        (self.generate)(ctx)
    }
}
