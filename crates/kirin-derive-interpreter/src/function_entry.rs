use kirin_derive_toolkit::context::DeriveContext;
use kirin_derive_toolkit::ir::{Data, Input};
use kirin_derive_toolkit::misc::from_str;
use kirin_derive_toolkit::prelude::darling;
use proc_macro2::TokenStream;
use quote::quote;

use crate::layout::InterpreterLayout;

const DEFAULT_INTERP_CRATE: &str = "::kirin_interpreter";
const DEFAULT_IR_CRATE: &str = "::kirin::ir";

pub fn do_derive_function_entry(input: &syn::DeriveInput) -> darling::Result<TokenStream> {
    let ir = Input::<InterpreterLayout>::from_derive_input(input)?;
    let interp_crate = parse_interpret_crate_path(input)?;
    let ir_crate: syn::Path = ir
        .attrs
        .crate_path
        .clone()
        .unwrap_or_else(|| from_str(DEFAULT_IR_CRATE));

    ir.compose()
        .add(move |ctx: &DeriveContext<'_, InterpreterLayout>| {
            emit_function_entry(ctx, &interp_crate, &ir_crate)
        })
        .build()
}

fn emit_function_entry(
    ctx: &DeriveContext<'_, InterpreterLayout>,
    interp_crate: &syn::Path,
    ir_crate: &syn::Path,
) -> darling::Result<Vec<TokenStream>> {
    validate_function_entry(ctx)?;

    let type_name = &ctx.meta.name;
    let mut impl_generics = ctx.meta.generics.clone();
    impl_generics
        .params
        .insert(0, syn::GenericParam::Type(syn::parse_quote!(__EntryL)));
    impl_generics
        .params
        .push(syn::GenericParam::Type(syn::parse_quote!(__EntryI)));
    impl_generics
        .params
        .push(syn::GenericParam::Type(syn::parse_quote!(__EntryF)));
    impl_generics
        .params
        .push(syn::GenericParam::Type(syn::parse_quote!(__EntryE)));
    impl_generics
        .params
        .push(syn::GenericParam::Type(syn::parse_quote!(__EntryV)));

    let (impl_generics, _, _) = impl_generics.split_for_impl();
    let (_, ty_generics, original_where) = ctx.meta.generics.split_for_impl();
    let callable_wrappers = collect_callable_wrappers(ctx);

    let mut predicates: Vec<syn::WherePredicate> = vec![
        syn::parse_quote! { __EntryL: #ir_crate::Dialect },
        syn::parse_quote! { __EntryE: ::core::convert::From<#interp_crate::InterpreterError> },
    ];
    for wrapper_ty in callable_wrappers {
        predicates.push(syn::parse_quote! {
            #wrapper_ty: #interp_crate::FunctionEntry<
                __EntryL,
                __EntryI,
                __EntryF,
                __EntryE,
                __EntryV
            >
        });
    }
    let extra_where: syn::WhereClause = syn::parse_quote! { where #(#predicates),* };
    let where_clause =
        kirin_derive_toolkit::codegen::combine_where_clauses(Some(&extra_where), original_where);

    let Data::Enum(data) = &ctx.input.data else {
        return Err(darling::Error::custom("expected enum input"));
    };

    let mut arms = Vec::new();
    for variant in &data.variants {
        let stmt_ctx = ctx
            .statements
            .get(&variant.name.to_string())
            .ok_or_else(|| darling::Error::custom("missing statement context"))?;
        let variant_name = &variant.name;
        let pattern = &stmt_ctx.pattern;
        let arm_pattern = if stmt_ctx.pattern.is_empty() {
            quote! { Self::#variant_name }
        } else {
            quote! { Self::#variant_name #pattern }
        };
        if is_entry_forwarding(ctx, stmt_ctx) {
            let binding = stmt_ctx
                .wrapper_binding
                .as_ref()
                .ok_or_else(|| darling::Error::custom("expected wrapper binding"))?;
            arms.push(quote! {
                #arm_pattern => #binding.enter_function_body(location, env, interp, args)
            });
        } else {
            arms.push(quote! {
                #arm_pattern => Err(__EntryE::from(
                    #interp_crate::InterpreterError::Custom("expected function body statement")
                ))
            });
        }
    }

    let body = if data.has_hidden_variants {
        quote! {
            match self {
                #(#arms,)*
                _ => unreachable!()
            }
        }
    } else {
        quote! {
            match self {
                #(#arms),*
            }
        }
    };

    Ok(vec![quote! {
        #[automatically_derived]
        impl #impl_generics #interp_crate::FunctionEntry<
            __EntryL,
            __EntryI,
            __EntryF,
            __EntryE,
            __EntryV
        > for #type_name #ty_generics #where_clause {
            fn enter_function_body(
                &self,
                location: #interp_crate::Location,
                env: #interp_crate::EnvIndex,
                interp: &mut __EntryI,
                args: #ir_crate::Product<__EntryV>,
            ) -> Result<__EntryF, __EntryE> {
                #body
            }
        }
    }])
}

fn validate_function_entry(ctx: &DeriveContext<'_, InterpreterLayout>) -> darling::Result<()> {
    if !matches!(ctx.input.data, Data::Enum(_)) {
        return Err(darling::Error::custom(
            "Cannot derive `FunctionEntry`: expected a wrapper enum",
        ));
    }
    let callable_wrappers = collect_callable_wrappers(ctx);
    if callable_wrappers.is_empty() {
        return Err(darling::Error::custom(
            "derive(FunctionEntry) requires at least one #[callable] wrapper variant",
        ));
    }
    Ok(())
}

fn is_entry_forwarding(
    ctx: &DeriveContext<'_, InterpreterLayout>,
    stmt_ctx: &kirin_derive_toolkit::context::StatementContext<'_, InterpreterLayout>,
) -> bool {
    let callable_all = ctx.input.extra_attrs.callable;
    let is_callable = callable_all || stmt_ctx.stmt.extra_attrs.callable;
    stmt_ctx.is_wrapper && is_callable
}

fn collect_callable_wrappers<'a>(
    ctx: &'a DeriveContext<'_, InterpreterLayout>,
) -> Vec<&'a syn::Type> {
    ctx.statements
        .values()
        .filter(|stmt_ctx| is_entry_forwarding(ctx, stmt_ctx))
        .filter_map(|stmt_ctx| stmt_ctx.wrapper_type)
        .collect()
}

fn parse_interpret_crate_path(input: &syn::DeriveInput) -> darling::Result<syn::Path> {
    let mut crate_path = None;
    for attr in &input.attrs {
        if !attr.path().is_ident("interpret") {
            continue;
        }
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("crate") {
                let value = meta.value()?;
                crate_path = Some(value.parse()?);
                Ok(())
            } else {
                Err(meta.error("unsupported attribute for #[interpret(...)]"))
            }
        })?;
    }
    Ok(crate_path.unwrap_or_else(|| from_str(DEFAULT_INTERP_CRATE)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use kirin_test_utils::rustfmt;

    fn generate_function_entry_code(input: syn::DeriveInput) -> String {
        let tokens = do_derive_function_entry(&input).expect("failed to generate FunctionEntry");
        rustfmt(tokens.to_string())
    }

    #[test]
    fn function_entry_for_callable_variants() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[wraps]
            #[kirin(type = T)]
            enum Lexical<T: CompileTimeValue> {
                #[callable]
                Function(Function<T>),
                Call(Call<T>),
                #[callable]
                Lambda(Lambda<T>),
                Return(Return<T>),
            }
        };
        insta::assert_snapshot!(generate_function_entry_code(input));
    }

    #[test]
    fn function_entry_rejects_without_callable() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[wraps]
            #[kirin(type = T)]
            enum Lexical<T> {
                Function(Function<T>),
                Return(Return<T>),
            }
        };
        let err = do_derive_function_entry(&input).unwrap_err().to_string();
        assert!(err.contains("requires at least one #[callable]"));
    }
}
