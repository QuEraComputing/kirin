use crate::context::{DeriveContext, StatementContext};
use crate::ir::Layout;
use crate::tokens::DelegationCall;
use proc_macro2::TokenStream;
use quote::{ToTokens, quote};

use super::MethodPattern;

/// Delegates a method call through the `#[wraps]` field to the wrapped type.
///
/// For wrapper variants: `<WrappedType as Trait>::method(binding, args...)`
/// Requires ALL variants to be `#[wraps]` unless `allow_non_wrapper` is set.
pub struct DelegateToWrapper<L: Layout> {
    trait_path: Box<dyn Fn(&DeriveContext<'_, L>) -> TokenStream>,
    method_name: syn::Ident,
    args: Vec<TokenStream>,
    require_all: bool,
}

impl<L: Layout> DelegateToWrapper<L> {
    pub fn new(
        trait_path: impl Fn(&DeriveContext<'_, L>) -> TokenStream + 'static,
        method_name: syn::Ident,
    ) -> Self {
        Self {
            trait_path: Box::new(trait_path),
            method_name,
            args: Vec::new(),
            require_all: false,
        }
    }

    pub fn args(mut self, args: Vec<TokenStream>) -> Self {
        self.args = args;
        self
    }

    /// Require ALL variants to be `#[wraps]`. Error if any variant is not.
    pub fn require_all(mut self) -> Self {
        self.require_all = true;
        self
    }

    fn delegation_body(
        &self,
        ctx: &DeriveContext<'_, L>,
        stmt_ctx: &StatementContext<'_, L>,
    ) -> darling::Result<TokenStream> {
        let (wrapper_ty, binding) = match (stmt_ctx.wrapper_type, &stmt_ctx.wrapper_binding) {
            (Some(ty), Some(binding)) => (ty, binding),
            _ => {
                if self.require_all {
                    return Err(darling::Error::custom(format!(
                        "Cannot delegate '{}' for variant '{}' without `#[wraps]`.",
                        self.method_name, stmt_ctx.stmt.name
                    ))
                    .with_span(&stmt_ctx.stmt.name));
                }
                return Err(darling::Error::custom("not a wrapper"));
            }
        };

        let trait_path = (self.trait_path)(ctx);
        let mut call_args = vec![binding.clone()];
        call_args.extend(self.args.iter().cloned());

        Ok(DelegationCall {
            wrapper_ty: quote! { #wrapper_ty },
            trait_path: quote! { #trait_path },
            trait_method: self.method_name.clone(),
            field: binding.clone(),
        }
        .to_token_stream())
    }
}

impl<L: Layout> MethodPattern<L> for DelegateToWrapper<L> {
    fn for_struct(
        &self,
        ctx: &DeriveContext<'_, L>,
        stmt_ctx: &StatementContext<'_, L>,
    ) -> darling::Result<TokenStream> {
        let body = self.delegation_body(ctx, stmt_ctx)?;
        let pattern = &stmt_ctx.pattern;
        Ok(quote! {
            let Self #pattern = self;
            #body
        })
    }

    fn for_variant(
        &self,
        ctx: &DeriveContext<'_, L>,
        stmt_ctx: &StatementContext<'_, L>,
    ) -> darling::Result<TokenStream> {
        self.delegation_body(ctx, stmt_ctx)
    }
}

/// Delegates method calls only for variants marked with a specific attribute.
///
/// Non-matching variants get a fallback body.
pub struct SelectiveDelegation<L: Layout> {
    trait_path: Box<dyn Fn(&DeriveContext<'_, L>) -> TokenStream>,
    method_name: syn::Ident,
    /// Attribute that marks a variant as forwarding (e.g., "callable").
    selector_attr: &'static str,
    /// Check if selector appears at the global (type) level.
    check_global: Box<dyn Fn(&DeriveContext<'_, L>) -> bool>,
    /// Body for non-matching variants.
    fallback: TokenStream,
}

impl<L: Layout> SelectiveDelegation<L> {
    pub fn new(
        trait_path: impl Fn(&DeriveContext<'_, L>) -> TokenStream + 'static,
        method_name: syn::Ident,
        selector_attr: &'static str,
        check_global: impl Fn(&DeriveContext<'_, L>) -> bool + 'static,
        fallback: TokenStream,
    ) -> Self {
        Self {
            trait_path: Box::new(trait_path),
            method_name,
            selector_attr,
            check_global: Box::new(check_global),
            fallback,
        }
    }

    fn is_selected(&self, ctx: &DeriveContext<'_, L>, stmt_ctx: &StatementContext<'_, L>) -> bool {
        let global = (self.check_global)(ctx);
        let local = stmt_ctx
            .stmt
            .raw_attrs
            .iter()
            .any(|a| a.path().is_ident(self.selector_attr));
        global || local
    }

    fn any_selected(&self, ctx: &DeriveContext<'_, L>) -> bool {
        let global = (self.check_global)(ctx);
        if global {
            return true;
        }
        ctx.statements.values().any(|stmt_ctx| {
            stmt_ctx
                .stmt
                .raw_attrs
                .iter()
                .any(|a| a.path().is_ident(self.selector_attr))
        })
    }
}

impl<L: Layout> MethodPattern<L> for SelectiveDelegation<L> {
    fn for_struct(
        &self,
        ctx: &DeriveContext<'_, L>,
        stmt_ctx: &StatementContext<'_, L>,
    ) -> darling::Result<TokenStream> {
        if stmt_ctx.is_wrapper && self.is_selected(ctx, stmt_ctx) {
            let (wrapper_ty, binding) = (
                stmt_ctx.wrapper_type.unwrap(),
                stmt_ctx.wrapper_binding.as_ref().unwrap(),
            );
            let trait_path = (self.trait_path)(ctx);
            let body = DelegationCall {
                wrapper_ty: quote! { #wrapper_ty },
                trait_path: quote! { #trait_path },
                trait_method: self.method_name.clone(),
                field: binding.clone(),
            }
            .to_token_stream();
            let pattern = &stmt_ctx.pattern;
            Ok(quote! {
                let Self #pattern = self;
                #body
            })
        } else {
            Ok(self.fallback.clone())
        }
    }

    fn for_variant(
        &self,
        ctx: &DeriveContext<'_, L>,
        stmt_ctx: &StatementContext<'_, L>,
    ) -> darling::Result<TokenStream> {
        let any_callable = self.any_selected(ctx);

        let is_forwarding = if any_callable {
            stmt_ctx.is_wrapper && self.is_selected(ctx, stmt_ctx)
        } else {
            // Backward compat: if no selector used anywhere, all wrappers forward
            stmt_ctx.is_wrapper
        };

        if is_forwarding {
            let (wrapper_ty, binding) = (
                stmt_ctx.wrapper_type.unwrap(),
                stmt_ctx.wrapper_binding.as_ref().unwrap(),
            );
            let trait_path = (self.trait_path)(ctx);
            Ok(DelegationCall {
                wrapper_ty: quote! { #wrapper_ty },
                trait_path: quote! { #trait_path },
                trait_method: self.method_name.clone(),
                field: binding.clone(),
            }
            .to_token_stream())
        } else {
            Ok(self.fallback.clone())
        }
    }
}
