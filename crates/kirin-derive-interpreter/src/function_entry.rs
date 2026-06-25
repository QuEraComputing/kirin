use kirin_derive_toolkit::context::DeriveContext;
use kirin_derive_toolkit::ir::{Data, Input};
use kirin_derive_toolkit::misc::from_str;
use kirin_derive_toolkit::prelude::darling;
use proc_macro2::TokenStream;
use quote::quote;

use crate::interpretable::parse_interpret_crate_path;
use crate::layout::InterpreterLayout;

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
    // Specialized on the *context* type `ValueContext<'__ctx, I>` (lifetime first, engine
    // type after), mirroring `Interpretable`.
    impl_generics
        .params
        .insert(0, syn::GenericParam::Lifetime(syn::parse_quote!('__ctx)));
    impl_generics
        .params
        .push(syn::GenericParam::Type(syn::parse_quote!(__EntryI)));

    let (impl_generics, _, _) = impl_generics.split_for_impl();
    let (_, ty_generics, original_where) = ctx.meta.generics.split_for_impl();
    let callable_wrappers = collect_callable_wrappers(ctx);

    let mut predicates: Vec<syn::WherePredicate> =
        vec![syn::parse_quote! { __EntryI: #interp_crate::Interp }];
    for wrapper_ty in callable_wrappers {
        predicates.push(syn::parse_quote! {
            #wrapper_ty: #interp_crate::FunctionEntry<#interp_crate::ValueContext<'__ctx, __EntryI>>
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
        if is_entry_forwarding(ctx, stmt_ctx) {
            let pattern = &stmt_ctx.pattern;
            let arm_pattern = if stmt_ctx.pattern.is_empty() {
                quote! { Self::#variant_name }
            } else {
                quote! { Self::#variant_name #pattern }
            };
            let binding = stmt_ctx
                .wrapper_binding
                .as_ref()
                .ok_or_else(|| darling::Error::custom("expected wrapper binding"))?;
            arms.push(quote! {
                #arm_pattern => #binding.function_entry(args, ctx)
            });
        } else {
            arms.push(quote! {
                Self::#variant_name { .. } => Err(<__EntryI as #interp_crate::Interp>::Error::from(
                    #interp_crate::InterpreterError::NotCallable(ctx.statement())
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
        impl #impl_generics #interp_crate::FunctionEntry<#interp_crate::ValueContext<'__ctx, __EntryI>> for #type_name #ty_generics #where_clause {
            fn function_entry(
                &self,
                args: #ir_crate::Product<<__EntryI as #interp_crate::Interp>::Value>,
                ctx: &mut #interp_crate::ValueContext<'__ctx, __EntryI>,
            ) -> Result<
                #interp_crate::FunctionBody<<__EntryI as #interp_crate::Interp>::Value>,
                <__EntryI as #interp_crate::Interp>::Error,
            > {
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
