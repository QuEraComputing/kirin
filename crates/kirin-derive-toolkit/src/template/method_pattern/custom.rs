use crate::context::{DeriveContext, StatementContext};
use crate::ir::Layout;
use proc_macro2::TokenStream;

use super::MethodPattern;

/// Closure-based method pattern for one-off custom logic.
pub struct Custom<L: Layout> {
    for_struct: Box<
        dyn Fn(&DeriveContext<'_, L>, &StatementContext<'_, L>) -> darling::Result<TokenStream>,
    >,
    for_variant: Box<
        dyn Fn(&DeriveContext<'_, L>, &StatementContext<'_, L>) -> darling::Result<TokenStream>,
    >,
}

impl<L: Layout> Custom<L> {
    /// Create a custom pattern with the same closure for both struct and variant cases.
    pub fn new(
        f: impl Fn(&DeriveContext<'_, L>, &StatementContext<'_, L>) -> darling::Result<TokenStream>
        + 'static
        + Clone,
    ) -> Self {
        let f2 = f.clone();
        Self {
            for_struct: Box::new(f),
            for_variant: Box::new(f2),
        }
    }

    /// Create a custom pattern with separate closures for struct vs variant cases.
    pub fn separate(
        for_struct: impl Fn(
            &DeriveContext<'_, L>,
            &StatementContext<'_, L>,
        ) -> darling::Result<TokenStream>
        + 'static,
        for_variant: impl Fn(
            &DeriveContext<'_, L>,
            &StatementContext<'_, L>,
        ) -> darling::Result<TokenStream>
        + 'static,
    ) -> Self {
        Self {
            for_struct: Box::new(for_struct),
            for_variant: Box::new(for_variant),
        }
    }
}

impl<L: Layout> MethodPattern<L> for Custom<L> {
    fn for_struct(
        &self,
        ctx: &DeriveContext<'_, L>,
        stmt_ctx: &StatementContext<'_, L>,
    ) -> darling::Result<TokenStream> {
        (self.for_struct)(ctx, stmt_ctx)
    }

    fn for_variant(
        &self,
        ctx: &DeriveContext<'_, L>,
        stmt_ctx: &StatementContext<'_, L>,
    ) -> darling::Result<TokenStream> {
        (self.for_variant)(ctx, stmt_ctx)
    }
}
