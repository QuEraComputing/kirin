mod layout;

pub use layout::EvalCallLayout;

use kirin_derive_toolkit::context::{DeriveContext, StatementContext};
use kirin_derive_toolkit::ir::Input;
use kirin_derive_toolkit::misc::from_str;
use kirin_derive_toolkit::prelude::darling;
use kirin_derive_toolkit::template::TraitImplTemplate;
use kirin_derive_toolkit::template::method_pattern::{AssocTypeSpec, Custom, MethodSpec};
use proc_macro2::TokenStream;
use quote::{ToTokens, format_ident, quote};

const DEFAULT_INTERP_CRATE: &str = "::kirin_interpreter";
const DEFAULT_IR_CRATE: &str = "::kirin::ir";

pub fn do_derive_eval_call(input: &syn::DeriveInput) -> darling::Result<TokenStream> {
    let ir = Input::<EvalCallLayout>::from_derive_input(input)?;

    let ir_crate: syn::Path = ir
        .attrs
        .crate_path
        .clone()
        .unwrap_or_else(|| from_str(DEFAULT_IR_CRATE));
    let type_name = ir.name.clone();
    let (_, ty_generics_raw, _) = ir.generics.split_for_impl();
    let ty_generics = ty_generics_raw.to_token_stream();

    let template = TraitImplTemplate::new(
        syn::parse_quote!(::kirin_interpreter::CallSemantics),
        from_str(DEFAULT_INTERP_CRATE),
    )
    .generics_modifier(|base| {
        let mut generics = base.clone();
        generics
            .params
            .insert(0, syn::GenericParam::Lifetime(syn::parse_quote!('__ir)));
        generics
            .params
            .push(syn::GenericParam::Type(syn::parse_quote!(__CallSemI)));
        generics
    })
    .trait_generics(|ctx| {
        let type_name = &ctx.meta.name;
        let (_, ty_generics, _) = ctx.meta.generics.split_for_impl();
        quote! { <'__ir, __CallSemI, #type_name #ty_generics> }
    })
    .where_clause(|ctx| {
        let interp_crate: syn::Path = from_str(DEFAULT_INTERP_CRATE);
        let type_name = &ctx.meta.name;
        let (_, ty_generics, _) = ctx.meta.generics.split_for_impl();

        let mut predicates: Vec<syn::WherePredicate> = vec![
            syn::parse_quote! { __CallSemI: #interp_crate::Interpreter<'__ir> },
            syn::parse_quote! { __CallSemI::Error: From<#interp_crate::InterpreterError> },
        ];

        let callable_wrappers = collect_callable_wrappers(ctx);
        let result_type = if let Some(first_ty) = callable_wrappers.first() {
            quote! { <#first_ty as #interp_crate::CallSemantics<'__ir, __CallSemI, #type_name #ty_generics>>::Result }
        } else {
            quote! { __CallSemI::Value }
        };

        for (i, wrapper_ty) in callable_wrappers.iter().enumerate() {
            if i == 0 {
                predicates.push(syn::parse_quote! {
                    #wrapper_ty: #interp_crate::CallSemantics<'__ir, __CallSemI, #type_name #ty_generics>
                });
            } else {
                predicates.push(syn::parse_quote! {
                    #wrapper_ty: #interp_crate::CallSemantics<'__ir, __CallSemI, #type_name #ty_generics, Result = #result_type>
                });
            }
        }

        Some(syn::parse_quote! { where #(#predicates),* })
    })
    .assoc_type(AssocTypeSpec::PerStatement {
        name: format_ident!("Result"),
        compute: Box::new(|ctx, _first_stmt| {
            let interp_crate: syn::Path = from_str(DEFAULT_INTERP_CRATE);
            let type_name = &ctx.meta.name;
            let (_, ty_generics, _) = ctx.meta.generics.split_for_impl();

            let callable_wrappers = collect_callable_wrappers(ctx);
            if let Some(first_ty) = callable_wrappers.first() {
                quote! { <#first_ty as #interp_crate::CallSemantics<'__ir, __CallSemI, #type_name #ty_generics>>::Result }
            } else {
                quote! { __CallSemI::Value }
            }
        }),
    })
    .method(MethodSpec {
        name: format_ident!("eval_call"),
        self_arg: quote! { &self },
        params: vec![
            quote! { interpreter: &mut __CallSemI },
            quote! { stage: &'__ir #ir_crate::StageInfo<#type_name #ty_generics> },
            quote! { callee: #ir_crate::SpecializedFunction },
            quote! { args: &[__CallSemI::Value] },
        ],
        return_type: Some(quote! { Result<Self::Result, __CallSemI::Error> }),
        pattern: Box::new(Custom::separate(
            // for_struct
            |_ctx, stmt_ctx| {
                let interp_crate: syn::Path = from_str(DEFAULT_INTERP_CRATE);
                if stmt_ctx.is_wrapper {
                    let pattern = &stmt_ctx.pattern;
                    let binding = stmt_ctx.wrapper_binding.as_ref().unwrap();
                    Ok(quote! {
                        let Self #pattern = self;
                        #binding.eval_call(interpreter, stage, callee, args)
                    })
                } else {
                    Ok(quote! {
                        Err(#interp_crate::InterpreterError::MissingEntry.into())
                    })
                }
            },
            // for_variant
            |ctx, stmt_ctx| {
                let interp_crate: syn::Path = from_str(DEFAULT_INTERP_CRATE);
                if is_call_forwarding(ctx, stmt_ctx) {
                    let binding = stmt_ctx.wrapper_binding.as_ref().unwrap();
                    Ok(quote! { #binding.eval_call(interpreter, stage, callee, args) })
                } else {
                    Ok(quote! { Err(#interp_crate::InterpreterError::MissingEntry.into()) })
                }
            },
        )),
    });

    ir.compose().add(template).build()
}

/// Determine if a variant should forward eval_call.
fn is_call_forwarding(
    ctx: &DeriveContext<'_, EvalCallLayout>,
    stmt_ctx: &StatementContext<'_, EvalCallLayout>,
) -> bool {
    let callable_all = ctx.input.extra_attrs.callable;
    let any_callable =
        callable_all || ctx.statements.values().any(|s| s.stmt.extra_attrs.callable);

    let is_callable = callable_all || stmt_ctx.stmt.extra_attrs.callable;

    if any_callable {
        stmt_ctx.is_wrapper && is_callable
    } else {
        // Backward compat: if no #[callable] used anywhere, all wrappers forward
        stmt_ctx.is_wrapper
    }
}

/// Collect wrapper types that should forward eval_call.
fn collect_callable_wrappers<'a>(ctx: &'a DeriveContext<'_, EvalCallLayout>) -> Vec<&'a syn::Type> {
    ctx.statements
        .values()
        .filter(|stmt_ctx| is_call_forwarding(ctx, stmt_ctx))
        .filter_map(|stmt_ctx| stmt_ctx.wrapper_type)
        .collect()
}
