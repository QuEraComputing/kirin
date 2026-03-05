mod layout;

pub use layout::EvalCallLayout;

use kirin_derive_toolkit::codegen::combine_where_clauses;
use kirin_derive_toolkit::context::{DeriveContext, StatementContext};
use kirin_derive_toolkit::ir::{self, Input};
use kirin_derive_toolkit::misc::from_str;
use kirin_derive_toolkit::prelude::darling;
use kirin_derive_toolkit::tokens::{MatchArm, MatchExpr, Method, TraitImpl};
use proc_macro2::TokenStream;
use quote::{ToTokens, format_ident, quote};

const DEFAULT_INTERP_CRATE: &str = "::kirin_interpreter";
const DEFAULT_IR_CRATE: &str = "::kirin::ir";

pub fn do_derive_eval_call(input: &syn::DeriveInput) -> darling::Result<TokenStream> {
    let ir = Input::<EvalCallLayout>::from_derive_input(input)?;
    ir.compose()
        .add(|ctx: &DeriveContext<'_, EvalCallLayout>| emit_call_semantics(ctx))
        .build()
}

fn interp_crate() -> syn::Path {
    from_str(DEFAULT_INTERP_CRATE)
}

fn ir_crate(ctx: &DeriveContext<'_, EvalCallLayout>) -> syn::Path {
    ctx.meta
        .crate_path
        .clone()
        .unwrap_or_else(|| from_str(DEFAULT_IR_CRATE))
}

fn emit_call_semantics(
    ctx: &DeriveContext<'_, EvalCallLayout>,
) -> darling::Result<Vec<TokenStream>> {
    match &ctx.input.data {
        ir::Data::Struct(data) => {
            let stmt_ctx = ctx
                .statements
                .get(&data.0.name.to_string())
                .ok_or_else(|| darling::Error::custom("missing statement context"))?;
            emit_struct(ctx, stmt_ctx).map(|t| vec![t])
        }
        ir::Data::Enum(data) => emit_enum(ctx, data).map(|t| vec![t]),
    }
}

fn emit_struct(
    ctx: &DeriveContext<'_, EvalCallLayout>,
    stmt_ctx: &StatementContext<'_, EvalCallLayout>,
) -> darling::Result<TokenStream> {
    let interp_crate = interp_crate();
    let ir_crate = ir_crate(ctx);
    let type_name = &ctx.meta.name;
    let (_, ty_generics_raw, orig_where) = ctx.meta.generics.split_for_impl();
    let ty_generics = ty_generics_raw.to_token_stream();

    let generics = add_interpreter_param(&ctx.meta.generics);
    let trait_path =
        quote! { #interp_crate::CallSemantics<'__ir, __CallSemI, #type_name #ty_generics> };

    if stmt_ctx.is_wrapper {
        let wrapper_ty = stmt_ctx.wrapper_type.unwrap();
        let pattern = &stmt_ctx.pattern;
        let binding = stmt_ctx.wrapper_binding.as_ref().unwrap();

        let extra_where: syn::WhereClause = syn::parse_quote! {
            where
                __CallSemI: #interp_crate::Interpreter<'__ir>,
                __CallSemI::Error: From<#interp_crate::InterpreterError>,
                #wrapper_ty: #interp_crate::CallSemantics<'__ir, __CallSemI, #type_name #ty_generics>
        };

        let result_type = quote! {
            <#wrapper_ty as #interp_crate::CallSemantics<'__ir, __CallSemI, #type_name #ty_generics>>::Result
        };

        let trait_impl = TraitImpl::new(generics, &trait_path, type_name)
            .type_generics(&ty_generics)
            .where_clause(combine_where_clauses(Some(&extra_where), orig_where))
            .assoc_type(format_ident!("Result"), &result_type)
            .method(Method {
                name: format_ident!("eval_call"),
                self_arg: quote! { &self },
                params: eval_call_params(&ir_crate, type_name, &ty_generics),
                return_type: Some(quote! { Result<Self::Result, __CallSemI::Error> }),
                body: quote! {
                    let Self #pattern = self;
                    #binding.eval_call(interpreter, stage, callee, args)
                },
            });

        Ok(quote! { #trait_impl })
    } else {
        let extra_where: syn::WhereClause = syn::parse_quote! {
            where
                __CallSemI: #interp_crate::Interpreter<'__ir>,
                __CallSemI::Error: From<#interp_crate::InterpreterError>
        };

        let trait_impl = TraitImpl::new(generics, &trait_path, type_name)
            .type_generics(ty_generics.clone())
            .where_clause(combine_where_clauses(Some(&extra_where), orig_where))
            .assoc_type(format_ident!("Result"), quote! { __CallSemI::Value })
            .method(Method {
                name: format_ident!("eval_call"),
                self_arg: quote! { &self },
                params: eval_call_params_prefixed(&ir_crate, type_name, &ty_generics),
                return_type: Some(quote! { Result<Self::Result, __CallSemI::Error> }),
                body: quote! {
                    Err(#interp_crate::InterpreterError::MissingEntry.into())
                },
            });

        Ok(quote! { #trait_impl })
    }
}

fn emit_enum(
    ctx: &DeriveContext<'_, EvalCallLayout>,
    data: &ir::DataEnum<EvalCallLayout>,
) -> darling::Result<TokenStream> {
    let interp_crate = interp_crate();
    let ir_crate = ir_crate(ctx);
    let type_name = &ctx.meta.name;
    let (_, ty_generics_raw, orig_where) = ctx.meta.generics.split_for_impl();
    let ty_generics = ty_generics_raw.to_token_stream();

    let generics = add_interpreter_param(&ctx.meta.generics);
    let callable_all = ctx.input.extra_attrs.callable;

    // Determine if #[callable] is used anywhere (enum-level or any variant).
    let any_callable = callable_all || ctx.statements.values().any(|s| s.stmt.extra_attrs.callable);

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

        let is_callable = callable_all || stmt_ctx.stmt.extra_attrs.callable;

        // A variant forwards eval_call if:
        // - No #[callable] used anywhere: fall back to #[wraps] (backward compat)
        // - #[callable] used: only callable wrappers forward
        let is_call_wrapper = if any_callable {
            stmt_ctx.is_wrapper && is_callable
        } else {
            stmt_ctx.is_wrapper
        };

        if is_call_wrapper {
            let wrapper_ty = stmt_ctx.wrapper_type.unwrap();
            wrapper_types.push(wrapper_ty);
            let binding = stmt_ctx.wrapper_binding.as_ref().unwrap();

            match_arms.push(MatchArm {
                pattern: quote! { Self::#variant_name #pattern },
                guard: None,
                body: quote! { #binding.eval_call(interpreter, stage, callee, args) },
            });
        } else if stmt_ctx.pattern.is_empty() {
            match_arms.push(MatchArm {
                pattern: quote! { Self::#variant_name },
                guard: None,
                body: quote! { Err(#interp_crate::InterpreterError::MissingEntry.into()) },
            });
        } else {
            match_arms.push(MatchArm {
                pattern: quote! { Self::#variant_name #pattern },
                guard: None,
                body: quote! { Err(#interp_crate::InterpreterError::MissingEntry.into()) },
            });
        }
    }

    let result_type = if let Some(first_wrapper) = wrapper_types.first() {
        quote! { <#first_wrapper as #interp_crate::CallSemantics<'__ir, __CallSemI, #type_name #ty_generics>>::Result }
    } else {
        quote! { __CallSemI::Value }
    };

    let where_bounds: Vec<TokenStream> = wrapper_types
        .iter()
        .enumerate()
        .map(|(i, ty)| {
            if i == 0 {
                quote! { #ty: #interp_crate::CallSemantics<'__ir, __CallSemI, #type_name #ty_generics> }
            } else {
                quote! { #ty: #interp_crate::CallSemantics<'__ir, __CallSemI, #type_name #ty_generics, Result = #result_type> }
            }
        })
        .collect();

    let extra_where: syn::WhereClause = syn::parse_quote! {
        where
            __CallSemI: #interp_crate::Interpreter<'__ir>,
            __CallSemI::Error: From<#interp_crate::InterpreterError>,
            #(#where_bounds),*
    };

    let trait_path =
        quote! { #interp_crate::CallSemantics<'__ir, __CallSemI, #type_name #ty_generics> };

    let match_expr = MatchExpr {
        subject: quote! { self },
        arms: match_arms,
    };

    let trait_impl = TraitImpl::new(generics, &trait_path, type_name)
        .type_generics(ty_generics.clone())
        .where_clause(combine_where_clauses(Some(&extra_where), orig_where))
        .assoc_type(format_ident!("Result"), &result_type)
        .method(Method {
            name: format_ident!("eval_call"),
            self_arg: quote! { &self },
            params: eval_call_params(&ir_crate, type_name, &ty_generics),
            return_type: Some(quote! { Result<Self::Result, __CallSemI::Error> }),
            body: quote! { #match_expr },
        });

    Ok(quote! { #trait_impl })
}

fn eval_call_params(
    ir_crate: &syn::Path,
    type_name: &syn::Ident,
    ty_generics: &TokenStream,
) -> Vec<TokenStream> {
    vec![
        quote! { interpreter: &mut __CallSemI },
        quote! { stage: &'__ir #ir_crate::StageInfo<#type_name #ty_generics> },
        quote! { callee: #ir_crate::SpecializedFunction },
        quote! { args: &[__CallSemI::Value] },
    ]
}

fn eval_call_params_prefixed(
    ir_crate: &syn::Path,
    type_name: &syn::Ident,
    ty_generics: &TokenStream,
) -> Vec<TokenStream> {
    vec![
        quote! { _interpreter: &mut __CallSemI },
        quote! { _stage: &'__ir #ir_crate::StageInfo<#type_name #ty_generics> },
        quote! { _callee: #ir_crate::SpecializedFunction },
        quote! { _args: &[__CallSemI::Value] },
    ]
}

fn add_interpreter_param(base: &syn::Generics) -> syn::Generics {
    let mut generics = base.clone();
    let lt_param: syn::LifetimeParam = syn::parse_quote! { '__ir };
    let param: syn::TypeParam = syn::parse_quote! { __CallSemI };
    generics
        .params
        .insert(0, syn::GenericParam::Lifetime(lt_param));
    generics.params.push(syn::GenericParam::Type(param));
    generics
}
