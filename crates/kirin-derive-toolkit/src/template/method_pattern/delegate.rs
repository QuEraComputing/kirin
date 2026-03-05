use crate::context::{DeriveContext, StatementContext};
use crate::ir::Layout;
use crate::tokens::DelegationCall;
use proc_macro2::TokenStream;
use quote::{ToTokens, quote};

use super::MethodPattern;

/// Delegates a method call through the `#[wraps]` field to the wrapped type.
///
/// Generates fully-qualified calls like `<WrappedType as Trait>::method(field, args...)`.
/// By default, non-wrapper variants produce an error; call [`require_all`](Self::require_all)
/// to make this an explicit compile-time check.
pub struct DelegateToWrapper<L: Layout> {
    /// Closure that resolves the fully-qualified trait path, accounting for
    /// `#[kirin(crate = ...)]` overrides.
    trait_path: Box<dyn Fn(&DeriveContext<'_, L>) -> TokenStream>,
    /// Trait method to call on the wrapped type.
    method_name: syn::Ident,
    /// Extra arguments to pass after the wrapped field binding.
    args: Vec<TokenStream>,
    /// When `true`, emit a darling error for any variant without `#[wraps]`.
    require_all: bool,
}

impl<L: Layout> DelegateToWrapper<L> {
    /// Create a delegation pattern for the given trait method.
    ///
    /// `trait_path` is a closure so it can incorporate `#[kirin(crate = ...)]`
    /// overrides from the derive context at expansion time.
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

    /// Set additional arguments to pass after the wrapped field binding.
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
/// Variants carrying the `selector_attr` (e.g., `#[callable]`) delegate through
/// their `#[wraps]` field. All other variants emit the `fallback` body instead.
/// For backward compatibility, if no variant in the entire enum carries the
/// selector attribute, all wrapper variants delegate unconditionally.
pub struct SelectiveDelegation<L: Layout> {
    /// Closure resolving the fully-qualified trait path.
    trait_path: Box<dyn Fn(&DeriveContext<'_, L>) -> TokenStream>,
    /// Trait method to call on the wrapped type.
    method_name: syn::Ident,
    /// Bare attribute name that marks a variant as forwarding (e.g., `"callable"`).
    selector_attr: &'static str,
    /// Check whether the selector applies at the global (type) level.
    check_global: Box<dyn Fn(&DeriveContext<'_, L>) -> bool>,
    /// Token stream emitted for non-matching variants.
    fallback: TokenStream,
}

impl<L: Layout> SelectiveDelegation<L> {
    /// Create a selective delegation pattern.
    ///
    /// * `trait_path` -- resolves the trait path from the derive context.
    /// * `method_name` -- the method to call on selected wrapper types.
    /// * `selector_attr` -- bare attribute name to match (e.g., `"callable"`).
    /// * `check_global` -- returns `true` if the selector applies type-wide.
    /// * `fallback` -- body emitted for variants that do not match.
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
