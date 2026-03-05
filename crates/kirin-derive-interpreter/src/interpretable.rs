use kirin_derive_toolkit::codegen::combine_where_clauses;
use kirin_derive_toolkit::context::DeriveContext;
use kirin_derive_toolkit::ir::{Input, StandardLayout};
use kirin_derive_toolkit::misc::from_str;
use kirin_derive_toolkit::prelude::darling;
use kirin_derive_toolkit::tokens::{MatchArm, MatchExpr, Method, TraitImpl};
use proc_macro2::TokenStream;
use quote::{ToTokens, quote};

const DEFAULT_INTERP_CRATE: &str = "::kirin_interpreter";

pub fn do_derive_interpretable(input: &syn::DeriveInput) -> darling::Result<TokenStream> {
    let ir = Input::<StandardLayout>::from_derive_input(input)?;
    ir.compose()
        .add(|ctx: &DeriveContext<'_, StandardLayout>| emit_interpretable(ctx))
        .build()
}

fn interp_crate() -> syn::Path {
    from_str(DEFAULT_INTERP_CRATE)
}

fn emit_interpretable(
    ctx: &DeriveContext<'_, StandardLayout>,
) -> darling::Result<Vec<TokenStream>> {
    // Validate: all variants must be wrappers
    let non_wrappers: Vec<_> = ctx
        .statements
        .values()
        .filter(|s| !s.is_wrapper)
        .map(|s| s.stmt.name.to_string())
        .collect();
    if !non_wrappers.is_empty() {
        return Err(darling::Error::custom(format!(
            "Cannot derive `Interpretable`: variant(s) {} are not `#[wraps]`. \
             Either implement `Interpretable` manually, or wrap each variant with `#[wraps]`.",
            non_wrappers.join(", "),
        )));
    }

    match &ctx.input.data {
        kirin_derive_toolkit::ir::Data::Struct(data) => {
            let stmt_ctx = ctx
                .statements
                .get(&data.0.name.to_string())
                .ok_or_else(|| darling::Error::custom("missing statement context"))?;
            emit_struct(ctx, stmt_ctx).map(|t| vec![t])
        }
        kirin_derive_toolkit::ir::Data::Enum(data) => emit_enum(ctx, data).map(|t| vec![t]),
    }
}

fn emit_struct(
    ctx: &DeriveContext<'_, StandardLayout>,
    stmt_ctx: &kirin_derive_toolkit::context::StatementContext<'_, StandardLayout>,
) -> darling::Result<TokenStream> {
    let interp_crate = interp_crate();
    let type_name = &ctx.meta.name;
    let (_, ty_generics_raw, orig_where) = ctx.meta.generics.split_for_impl();
    let ty_generics = ty_generics_raw.to_token_stream();

    let impl_generics = add_interpreter_params(&ctx.meta.generics);
    let wrapper_ty = stmt_ctx.wrapper_type.unwrap();
    let pattern = &stmt_ctx.pattern;
    let binding = stmt_ctx.wrapper_binding.as_ref().unwrap();

    let extra_where: syn::WhereClause = syn::parse_quote! {
        where
            __InterpI: #interp_crate::Interpreter<'__ir>,
            __InterpL: ::kirin_ir::Dialect,
            #wrapper_ty: #interp_crate::Interpretable<'__ir, __InterpI, __InterpL>
    };

    let trait_impl = TraitImpl::new(
        impl_generics,
        quote! { #interp_crate::Interpretable<'__ir, __InterpI, __InterpL> },
        type_name,
    )
    .type_generics(ty_generics)
    .where_clause(combine_where_clauses(Some(&extra_where), orig_where))
    .method(Method {
        name: syn::parse_quote! { interpret },
        self_arg: quote! { &self },
        params: vec![quote! { interpreter: &mut __InterpI }],
        return_type: Some(
            quote! { Result<#interp_crate::Continuation<__InterpI::Value, __InterpI::Ext>, __InterpI::Error> },
        ),
        body: quote! {
            let Self #pattern = self;
            #binding.interpret(interpreter)
        },
    });

    Ok(quote! { #trait_impl })
}

fn emit_enum(
    ctx: &DeriveContext<'_, StandardLayout>,
    data: &kirin_derive_toolkit::ir::DataEnum<StandardLayout>,
) -> darling::Result<TokenStream> {
    let interp_crate = interp_crate();
    let type_name = &ctx.meta.name;
    let (_, ty_generics_raw, orig_where) = ctx.meta.generics.split_for_impl();
    let ty_generics = ty_generics_raw.to_token_stream();

    let impl_generics = add_interpreter_params(&ctx.meta.generics);

    let mut wrapper_types: Vec<&syn::Type> = Vec::new();
    let mut match_arms = Vec::new();

    for variant in &data.variants {
        let stmt_ctx = ctx
            .statements
            .get(&variant.name.to_string())
            .ok_or_else(|| {
                darling::Error::custom(format!("missing statement context for '{}'", variant.name))
            })?;
        let variant_name = &variant.name;
        let pattern = &stmt_ctx.pattern;
        let wrapper_ty = stmt_ctx.wrapper_type.unwrap();
        wrapper_types.push(wrapper_ty);
        let binding = stmt_ctx.wrapper_binding.as_ref().unwrap();

        match_arms.push(MatchArm {
            pattern: quote! { Self::#variant_name #pattern },
            guard: None,
            body: quote! { #binding.interpret(interpreter) },
        });
    }

    let where_bounds: Vec<TokenStream> = wrapper_types
        .iter()
        .map(|ty| {
            quote! { #ty: #interp_crate::Interpretable<'__ir, __InterpI, __InterpL> }
        })
        .collect();

    let extra_where: syn::WhereClause = syn::parse_quote! {
        where
            __InterpI: #interp_crate::Interpreter<'__ir>,
            __InterpL: ::kirin_ir::Dialect,
            #(#where_bounds),*
    };

    let match_expr = MatchExpr {
        subject: quote! { self },
        arms: match_arms,
    };

    let trait_impl = TraitImpl::new(
        impl_generics,
        quote! { #interp_crate::Interpretable<'__ir, __InterpI, __InterpL> },
        type_name,
    )
    .type_generics(ty_generics)
    .where_clause(combine_where_clauses(Some(&extra_where), orig_where))
    .method(Method {
        name: syn::parse_quote! { interpret },
        self_arg: quote! { &self },
        params: vec![quote! { interpreter: &mut __InterpI }],
        return_type: Some(
            quote! { Result<#interp_crate::Continuation<__InterpI::Value, __InterpI::Ext>, __InterpI::Error> },
        ),
        body: quote! { #match_expr },
    });

    Ok(quote! { #trait_impl })
}

fn add_interpreter_params(base: &syn::Generics) -> syn::Generics {
    let mut generics = base.clone();
    generics
        .params
        .insert(0, syn::GenericParam::Lifetime(syn::parse_quote!('__ir)));
    generics
        .params
        .push(syn::GenericParam::Type(syn::parse_quote!(__InterpI)));
    generics
        .params
        .push(syn::GenericParam::Type(syn::parse_quote!(__InterpL)));
    generics
}
