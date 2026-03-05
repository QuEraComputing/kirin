use kirin_derive_toolkit::ir::{Input, StandardLayout};
use kirin_derive_toolkit::misc::from_str;
use kirin_derive_toolkit::prelude::darling;
use kirin_derive_toolkit::template::TraitImplTemplate;
use kirin_derive_toolkit::template::method_pattern::{Custom, MethodSpec};
use proc_macro2::TokenStream;
use quote::quote;

const DEFAULT_INTERP_CRATE: &str = "::kirin_interpreter";

pub fn do_derive_interpretable(input: &syn::DeriveInput) -> darling::Result<TokenStream> {
    let ir = Input::<StandardLayout>::from_derive_input(input)?;
    let interp_crate: syn::Path = from_str(DEFAULT_INTERP_CRATE);

    let template = TraitImplTemplate::new(
        syn::parse_quote!(::kirin_interpreter::Interpretable),
        interp_crate.clone(),
    )
    .generics_modifier(|base| {
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
    })
    .trait_generics(|_ctx| quote! { <'__ir, __InterpI, __InterpL> })
    .where_clause({
        let interp_crate = interp_crate.clone();
        move |ctx| {
            let mut predicates: Vec<syn::WherePredicate> = vec![
                syn::parse_quote! { __InterpI: #interp_crate::Interpreter<'__ir> },
                syn::parse_quote! { __InterpL: ::kirin_ir::Dialect },
            ];
            for stmt_ctx in ctx.statements.values() {
                if let Some(wrapper_ty) = stmt_ctx.wrapper_type {
                    predicates.push(syn::parse_quote! {
                        #wrapper_ty: #interp_crate::Interpretable<'__ir, __InterpI, __InterpL>
                    });
                }
            }
            Some(syn::parse_quote! { where #(#predicates),* })
        }
    })
    .validate(|ctx| {
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
        Ok(())
    })
    .method(MethodSpec {
        name: syn::parse_quote!(interpret),
        self_arg: quote! { &self },
        params: vec![quote! { interpreter: &mut __InterpI }],
        return_type: Some({
            let interp_crate = interp_crate.clone();
            quote! { Result<#interp_crate::Continuation<__InterpI::Value, __InterpI::Ext>, __InterpI::Error> }
        }),
        pattern: Box::new(Custom::new(|_ctx, stmt_ctx| {
            let binding = stmt_ctx
                .wrapper_binding
                .as_ref()
                .ok_or_else(|| darling::Error::custom("expected wrapper binding"))?;
            Ok(quote! { #binding.interpret(interpreter) })
        })),
    });

    ir.compose().add(template).build()
}
