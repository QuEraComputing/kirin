use kirin_derive_toolkit::context::DeriveContext;
use kirin_derive_toolkit::ir::{Data, Input, StandardLayout};
use kirin_derive_toolkit::misc::from_str;
use kirin_derive_toolkit::prelude::darling;
use proc_macro2::TokenStream;
use quote::quote;

pub const DEFAULT_INTERP_CRATE: &str = "::kirin_interpreter";

pub fn do_derive_interpretable(input: &syn::DeriveInput) -> darling::Result<TokenStream> {
    let ir = Input::<StandardLayout>::from_derive_input(input)?;
    let interp_crate = parse_interpret_crate_path(input)?;

    ir.compose()
        .add(move |ctx: &DeriveContext<'_, StandardLayout>| emit_interpretable(ctx, &interp_crate))
        .build()
}

fn emit_interpretable(
    ctx: &DeriveContext<'_, StandardLayout>,
    interp_crate: &syn::Path,
) -> darling::Result<Vec<TokenStream>> {
    validate_global_wrapper(ctx)?;

    let type_name = &ctx.meta.name;
    let mut impl_generics = ctx.meta.generics.clone();
    // The dialect trait is specialized on the engine type `__InterpI` and the
    // forward-value semantics marker; the delegating wrapper impl is generic over
    // the engine.
    impl_generics
        .params
        .push(syn::GenericParam::Type(syn::parse_quote!(__InterpI)));

    let (impl_generics, _, _) = impl_generics.split_for_impl();
    let (_, ty_generics, original_where) = ctx.meta.generics.split_for_impl();

    let mut predicates: Vec<syn::WherePredicate> =
        vec![syn::parse_quote! { __InterpI: #interp_crate::Interp }];
    for stmt_ctx in ctx.statements.values() {
        if let Some(wrapper_ty) = stmt_ctx.wrapper_type {
            predicates.push(syn::parse_quote! {
                #wrapper_ty: #interp_crate::Interpretable<__InterpI, #interp_crate::ForwardEval>
            });
        }
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
        let wrapper_ty = stmt_ctx
            .wrapper_type
            .ok_or_else(|| darling::Error::custom("expected wrapper type"))?;
        let binding = stmt_ctx
            .wrapper_binding
            .as_ref()
            .ok_or_else(|| darling::Error::custom("expected wrapper binding"))?;
        let variant_name = &variant.name;
        let pattern = &stmt_ctx.pattern;
        let arm_pattern = if stmt_ctx.pattern.is_empty() {
            quote! { Self::#variant_name }
        } else {
            quote! { Self::#variant_name #pattern }
        };
        arms.push(quote! {
            #arm_pattern => <#wrapper_ty as #interp_crate::Interpretable<__InterpI, #interp_crate::ForwardEval>>::interpret(
                #binding, interp,
            )
        });
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
        impl #impl_generics #interp_crate::Interpretable<__InterpI, #interp_crate::ForwardEval> for #type_name #ty_generics #where_clause {
            fn interpret(
                &self,
                interp: &mut __InterpI,
            ) -> Result<
                <__InterpI as #interp_crate::Interp>::Effect,
                <__InterpI as #interp_crate::Interp>::Error,
            > {
                #body
            }
        }
    }])
}

fn validate_global_wrapper<L: kirin_derive_toolkit::ir::Layout>(
    ctx: &kirin_derive_toolkit::context::DeriveContext<'_, L>,
) -> darling::Result<()> {
    if !matches!(ctx.input.data, Data::Enum(_)) {
        return Err(darling::Error::custom(
            "Cannot derive `Interpretable`: only wrapper enums are supported (use `#[wraps]` at the enum level or on every variant).",
        ));
    }

    let non_wrappers: Vec<_> = ctx
        .statements
        .values()
        .filter(|s| !s.is_wrapper)
        .map(|s| s.stmt.name.to_string())
        .collect();
    if !non_wrappers.is_empty() {
        return Err(darling::Error::custom(format!(
            "Cannot derive `Interpretable`: variant(s) {} are not wrappers. Add `#[wraps]` to each variant (or at the enum level if all variants wrap their first field).",
            non_wrappers.join(", "),
        )));
    }
    Ok(())
}

pub fn parse_interpret_crate_path(input: &syn::DeriveInput) -> darling::Result<syn::Path> {
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

    fn generate_interpretable_code(input: syn::DeriveInput) -> String {
        let tokens = do_derive_interpretable(&input).expect("failed to generate Interpretable");
        rustfmt(tokens.to_string())
    }

    #[test]
    fn interpretable_enum_level_wraps() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[wraps]
            #[kirin(type = T)]
            enum Lexical<T: CompileTimeValue> {
                Function(Function<T>),
                Call(Call<T>),
                Return(Return<T>),
            }
        };
        insta::assert_snapshot!(generate_interpretable_code(input));
    }

    #[test]
    fn interpretable_rejects_non_wrapper_variants() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(type = T)]
            enum Mixed<T> {
                #[wraps]
                Function(Function<T>),
                Inline { value: SSAValue },
            }
        };
        let err = do_derive_interpretable(&input).unwrap_err().to_string();
        assert!(err.contains("not wrappers"));
    }
}
