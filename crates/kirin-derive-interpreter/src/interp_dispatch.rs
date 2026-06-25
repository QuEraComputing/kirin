//! Code generation for `#[derive(InterpDispatch)]` on stage enums.
//!
//! Generates a monomorphic `InterpDispatch<I>` implementation that delegates
//! each stage variant to the blanket `InterpDispatch` impl on its
//! `StageInfo<L>`, mirroring `#[derive(ParseDispatch)]` for parsing.

use kirin_derive_toolkit::stage::{self, StageVariantInfo};
use proc_macro2::TokenStream;
use quote::quote;
use syn::DeriveInput;

const DEFAULT_INTERP_CRATE: &str = "::kirin_interpreter";

/// Extracts the `#[stage(interp_crate = "...")]` override from attributes.
fn parse_interp_crate_path(attrs: &[syn::Attribute]) -> Result<String, syn::Error> {
    for attr in attrs {
        if !attr.path().is_ident("stage") {
            continue;
        }
        let mut crate_path = None;
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("interp_crate") {
                let value = meta.value()?;
                let lit: syn::LitStr = value.parse()?;
                crate_path = Some(lit.value());
                Ok(())
            } else {
                stage::skip_meta_value(&meta);
                Ok(())
            }
        })?;
        if let Some(path) = crate_path {
            return Ok(path);
        }
    }
    Ok(DEFAULT_INTERP_CRATE.to_string())
}

pub fn generate(input: &DeriveInput) -> Result<TokenStream, syn::Error> {
    let variants = stage::parse_stage_variants(input)?;

    let ir_crate_str = stage::parse_ir_crate_path(&input.attrs)?;
    let ir_crate: syn::Path = syn::parse_str(&ir_crate_str)
        .map_err(|e| syn::Error::new_spanned(input, format!("invalid IR crate path: {e}")))?;

    let interp_crate_str = parse_interp_crate_path(&input.attrs)?;
    let interp_crate: syn::Path = syn::parse_str(&interp_crate_str).map_err(|e| {
        syn::Error::new_spanned(input, format!("invalid interpreter crate path: {e}"))
    })?;

    let enum_ident = &input.ident;
    let mut impl_generics = input.generics.clone();
    impl_generics
        .params
        .push(syn::GenericParam::Type(syn::parse_quote!(__Ctx)));
    let (impl_generics, _, _) = impl_generics.split_for_impl();
    let (_, ty_generics, original_where) = input.generics.split_for_impl();

    let mut predicates: Vec<syn::WherePredicate> =
        vec![syn::parse_quote! { __Ctx: #interp_crate::InterpretCtx }];
    for v in &variants {
        let dialect_ty = &v.dialect_ty;
        // Dispatch is keyed on the *context type* `__Ctx`, not the engine: the
        // engine builds its context and passes it in. So each language need only be
        // interpretable in that context — no higher-ranked GAT projection (which
        // would spuriously require `'static`); the forward engines instantiate
        // `__Ctx = ValueContext<'_, I>` in their `FrameDriver` bound.
        predicates.push(syn::parse_quote! {
            #dialect_ty: #interp_crate::Interpretable<__Ctx> + #interp_crate::FunctionEntry<__Ctx>
        });
    }
    let mut where_clause = original_where.cloned().unwrap_or_else(|| syn::WhereClause {
        where_token: Default::default(),
        predicates: Default::default(),
    });
    where_clause.predicates.extend(predicates);

    let statement_arms = build_arms(&variants, enum_ident, |_| {
        quote! {
            #interp_crate::InterpDispatch::dispatch_statement(
                stage_info, statement, ctx,
            )
        }
    });
    let entry_arms = build_arms(&variants, enum_ident, |_| {
        quote! {
            #interp_crate::InterpDispatch::dispatch_function_entry(
                stage_info, body, args, ctx,
            )
        }
    });

    Ok(quote! {
        #[automatically_derived]
        impl #impl_generics #interp_crate::InterpDispatch<__Ctx> for #enum_ident #ty_generics
        #where_clause
        {
            fn dispatch_statement(
                &self,
                statement: #ir_crate::Statement,
                ctx: &mut __Ctx,
            ) -> Result<
                <__Ctx as #interp_crate::InterpretCtx>::Effect,
                <__Ctx as #interp_crate::InterpretCtx>::Error,
            > {
                match self {
                    #statement_arms
                }
            }

            fn dispatch_function_entry(
                &self,
                body: #ir_crate::Statement,
                args: #ir_crate::Product<<__Ctx as #interp_crate::InterpretCtx>::Value>,
                ctx: &mut __Ctx,
            ) -> Result<
                #interp_crate::FunctionBody<<__Ctx as #interp_crate::InterpretCtx>::Value>,
                <__Ctx as #interp_crate::InterpretCtx>::Error,
            > {
                match self {
                    #entry_arms
                }
            }
        }
    })
}

fn build_arms(
    variants: &[StageVariantInfo],
    enum_ident: &syn::Ident,
    body: impl Fn(&StageVariantInfo) -> TokenStream,
) -> TokenStream {
    let mut arms = TokenStream::new();
    for v in variants {
        let ident = &v.ident;
        let call = body(v);
        arms.extend(quote! {
            #enum_ident::#ident(stage_info) => { #call }
        });
    }
    arms
}
