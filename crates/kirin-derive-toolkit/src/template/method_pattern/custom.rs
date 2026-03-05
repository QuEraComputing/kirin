use crate::context::{DeriveContext, StatementContext};
use crate::ir::Layout;
use proc_macro2::TokenStream;

use super::MethodPattern;

/// Closure-based [`MethodPattern`] for one-off custom logic.
///
/// Use this when the code generation logic does not fit any of the built-in patterns
/// (e.g., [`BoolProperty`](super::BoolProperty), [`DelegateToWrapper`](super::DelegateToWrapper)).
///
/// # Examples
///
/// ```ignore
/// // Same logic for struct and variant:
/// let pattern = Custom::new(|ctx, stmt| Ok(quote! { todo!() }));
///
/// // Different logic per case:
/// let pattern = Custom::separate(
///     |ctx, stmt| Ok(quote! { /* struct body */ }),
///     |ctx, stmt| Ok(quote! { /* variant arm body */ }),
/// );
/// ```
pub struct Custom<L: Layout> {
    for_struct: Box<
        dyn Fn(&DeriveContext<'_, L>, &StatementContext<'_, L>) -> darling::Result<TokenStream>,
    >,
    for_variant: Box<
        dyn Fn(&DeriveContext<'_, L>, &StatementContext<'_, L>) -> darling::Result<TokenStream>,
    >,
}

impl<L: Layout> Custom<L> {
    /// Create a custom pattern that uses the same closure for both struct and variant cases.
    ///
    /// The closure is cloned internally so that the struct path and variant path
    /// each hold an independent copy.
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

    /// Create a custom pattern with separate closures for the struct and variant cases.
    ///
    /// Use this when the struct body needs different code than the enum match arm
    /// body (e.g., the struct case destructures `self` while the variant case does not).
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
