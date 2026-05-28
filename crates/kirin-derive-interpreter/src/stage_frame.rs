//! Code generation for `#[derive(StageFrame)]`.
//!
//! Two modes, selected by the presence of `#[stage_frame(stage = StageEnum)]`
//! on the enum:
//!
//! - **Single-language mode** (no enum-level `stage_frame` attribute): the
//!   derive expects exactly one variant whose inner type's path ends in
//!   `StandardFrame`. The generated impl ignores the stage parameter and
//!   delegates construction to that `StandardFrame<L, V, T>`. The enum is
//!   typically generic in `L: Dialect`, `V`, and `T`.
//!
//! - **Multi-stage mode** (`#[stage_frame(stage = StageEnum)]` present):
//!   each enum variant is matched, by variant name, against the same-named
//!   variant of `StageEnum`. The generated impl pattern-matches the runtime
//!   `&StageEnum` and dispatches to the per-variant inner frame's own
//!   `StageFrame` impl.
//!
//! In both modes the generated impl is `impl<.., S> StageFrame<S, V>` (with
//! `S` either generic or pinned to the user's stage enum), with `type Error
//! = ::core::convert::Infallible`.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{GenericArgument, PathArguments, Type};

const DEFAULT_INTERP_CRATE: &str = "::kirin_interpreter";

pub fn do_derive_stage_frame(input: &syn::DeriveInput) -> syn::Result<TokenStream> {
    let interp_crate = parse_interpret_crate_path(input)?;

    let syn::Data::Enum(data) = &input.data else {
        return Err(syn::Error::new_spanned(
            input,
            "`#[derive(StageFrame)]` only supports enum inputs",
        ));
    };

    let stage_enum = parse_stage_attribute(input)?;
    if let Some(stage_path) = stage_enum {
        emit_multi_stage(input, data, &interp_crate, &stage_path)
    } else {
        emit_single_language(input, data, &interp_crate)
    }
}

fn emit_single_language(
    input: &syn::DeriveInput,
    data: &syn::DataEnum,
    interp_crate: &syn::Path,
) -> syn::Result<TokenStream> {
    let type_name = &input.ident;

    let standard_variant = data
        .variants
        .iter()
        .find_map(|v| standard_frame_args(&v.fields))
        .ok_or_else(|| {
            syn::Error::new_spanned(
                input,
                "`#[derive(StageFrame)]` (single-language mode) requires a variant whose inner \
                 type is `StandardFrame<L, V, T>`. Add `#[stage_frame(stage = StageEnum)]` to use \
                 multi-stage mode.",
            )
        })?;

    let (l_ty, v_ty, t_ty) = standard_variant;

    let mut generics = input.generics.clone();
    generics
        .params
        .push(syn::GenericParam::Type(syn::parse_quote!(__StageFrameS)));
    let (impl_generics, _, _) = generics.split_for_impl();
    let (_, ty_generics, where_clause) = input.generics.split_for_impl();

    Ok(quote! {
        #[automatically_derived]
        impl #impl_generics #interp_crate::StageFrame<__StageFrameS, #v_ty>
            for #type_name #ty_generics
        #where_clause
        {
            type Error = ::core::convert::Infallible;

            fn from_function_invocation(
                stage_info: &__StageFrameS,
                invocation: #interp_crate::FunctionInvocation<#v_ty>,
            ) -> ::core::result::Result<Self, Self::Error> {
                <#interp_crate::StandardFrame<#l_ty, #v_ty, #t_ty>
                    as #interp_crate::StageFrame<__StageFrameS, #v_ty>>::from_function_invocation(
                        stage_info, invocation,
                    )
                    .map(::core::convert::Into::into)
            }

            fn from_block(
                stage_info: &__StageFrameS,
                stage: ::kirin::ir::CompileStage,
                block: ::kirin::ir::Block,
                env: #interp_crate::EnvIndex,
                args: ::kirin::ir::Product<#v_ty>,
            ) -> ::core::result::Result<Self, Self::Error> {
                <#interp_crate::StandardFrame<#l_ty, #v_ty, #t_ty>
                    as #interp_crate::StageFrame<__StageFrameS, #v_ty>>::from_block(
                        stage_info, stage, block, env, args,
                    )
                    .map(::core::convert::Into::into)
            }
        }
    })
}

fn emit_multi_stage(
    input: &syn::DeriveInput,
    data: &syn::DataEnum,
    interp_crate: &syn::Path,
    stage_path: &syn::Path,
) -> syn::Result<TokenStream> {
    let type_name = &input.ident;

    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    if data.variants.is_empty() {
        return Err(syn::Error::new_spanned(
            input,
            "`#[derive(StageFrame)]` (multi-stage mode) requires at least one variant",
        ));
    }

    let mut value_ty: Option<&Type> = None;
    let mut invocation_arms = Vec::new();
    let mut block_arms = Vec::new();

    for variant in &data.variants {
        let inner = single_field_type(&variant.fields).ok_or_else(|| {
            syn::Error::new_spanned(
                variant,
                "`#[derive(StageFrame)]` requires each variant to wrap exactly one field",
            )
        })?;
        let v_ty = nth_generic_arg_type(inner, 1).ok_or_else(|| {
            syn::Error::new_spanned(
                inner,
                "`#[derive(StageFrame)]` could not find the value-type generic argument on this variant",
            )
        })?;

        if let Some(prev) = value_ty
            && !types_equal(prev, v_ty)
        {
            return Err(syn::Error::new_spanned(
                inner,
                "`#[derive(StageFrame)]` variants must share the same value-type generic argument",
            ));
        }
        value_ty = Some(v_ty);

        let variant_name = &variant.ident;
        invocation_arms.push(quote! {
            #stage_path::#variant_name(_) => <#inner as #interp_crate::StageFrame<#stage_path, #v_ty>>::from_function_invocation(stage_info, invocation).map(::core::convert::Into::into)
        });
        block_arms.push(quote! {
            #stage_path::#variant_name(_) => <#inner as #interp_crate::StageFrame<#stage_path, #v_ty>>::from_block(stage_info, stage, block, env, args).map(::core::convert::Into::into)
        });
    }

    let v_ty = value_ty.unwrap();

    Ok(quote! {
        #[automatically_derived]
        impl #impl_generics #interp_crate::StageFrame<#stage_path, #v_ty>
            for #type_name #ty_generics
        #where_clause
        {
            type Error = ::core::convert::Infallible;

            fn from_function_invocation(
                stage_info: &#stage_path,
                invocation: #interp_crate::FunctionInvocation<#v_ty>,
            ) -> ::core::result::Result<Self, Self::Error> {
                match stage_info {
                    #(#invocation_arms),*
                }
            }

            fn from_block(
                stage_info: &#stage_path,
                stage: ::kirin::ir::CompileStage,
                block: ::kirin::ir::Block,
                env: #interp_crate::EnvIndex,
                args: ::kirin::ir::Product<#v_ty>,
            ) -> ::core::result::Result<Self, Self::Error> {
                match stage_info {
                    #(#block_arms),*
                }
            }
        }
    })
}

fn standard_frame_args(fields: &syn::Fields) -> Option<(Type, Type, Type)> {
    let ty = single_field_type(fields)?;
    let path = match ty {
        Type::Path(p) => p,
        _ => return None,
    };
    let last = path.path.segments.last()?;
    if last.ident != "StandardFrame" {
        return None;
    }
    let PathArguments::AngleBracketed(args) = &last.arguments else {
        return None;
    };
    let mut it = args.args.iter().filter_map(|a| match a {
        GenericArgument::Type(t) => Some(t.clone()),
        _ => None,
    });
    let l = it.next()?;
    let v = it.next()?;
    let t = it.next()?;
    Some((l, v, t))
}

fn single_field_type(fields: &syn::Fields) -> Option<&Type> {
    match fields {
        syn::Fields::Unnamed(unnamed) if unnamed.unnamed.len() == 1 => {
            Some(&unnamed.unnamed.first().unwrap().ty)
        }
        syn::Fields::Named(named) if named.named.len() == 1 => {
            Some(&named.named.first().unwrap().ty)
        }
        _ => None,
    }
}

fn nth_generic_arg_type(ty: &Type, idx: usize) -> Option<&Type> {
    let Type::Path(p) = ty else { return None };
    let last = p.path.segments.last()?;
    let PathArguments::AngleBracketed(args) = &last.arguments else {
        return None;
    };
    args.args
        .iter()
        .filter_map(|a| match a {
            GenericArgument::Type(t) => Some(t),
            _ => None,
        })
        .nth(idx)
}

fn types_equal(a: &Type, b: &Type) -> bool {
    quote!(#a).to_string() == quote!(#b).to_string()
}

fn parse_stage_attribute(input: &syn::DeriveInput) -> syn::Result<Option<syn::Path>> {
    let mut stage = None;
    for attr in &input.attrs {
        if !attr.path().is_ident("stage_frame") {
            continue;
        }
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("stage") {
                let value = meta.value()?;
                stage = Some(value.parse()?);
                Ok(())
            } else {
                Err(meta.error("unsupported attribute for #[stage_frame(...)]"))
            }
        })?;
    }
    Ok(stage)
}

fn parse_interpret_crate_path(input: &syn::DeriveInput) -> syn::Result<syn::Path> {
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
    Ok(crate_path.unwrap_or_else(|| syn::parse_str(DEFAULT_INTERP_CRATE).unwrap()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use kirin_test_utils::rustfmt;

    fn generate(input: syn::DeriveInput) -> String {
        let tokens = do_derive_stage_frame(&input).expect("failed to generate StageFrame");
        rustfmt(tokens.to_string())
    }

    #[test]
    fn single_language_frame() {
        let input: syn::DeriveInput = syn::parse_quote! {
            enum ToyFrame<L: Dialect, V, T = ConcreteBlockTransfer<V>> {
                Standard(StandardFrame<L, V, T>),
                Scf(ScfFrame<L, ArithType, V, T>),
            }
        };
        insta::assert_snapshot!(generate(input));
    }

    #[test]
    fn multi_stage_frame() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[stage_frame(stage = Stage)]
            enum ToyStageFrame<V, T = ConcreteBlockTransfer<V>> {
                Source(ToyFrame<HighLevel, V, T>),
                Lowered(ToyFrame<LowLevel, V, T>),
            }
        };
        insta::assert_snapshot!(generate(input));
    }

    #[test]
    fn missing_standard_variant_errors() {
        let input: syn::DeriveInput = syn::parse_quote! {
            enum BadFrame<L: Dialect, V, T> {
                Scf(ScfFrame<L, ArithType, V, T>),
            }
        };
        let err = do_derive_stage_frame(&input).unwrap_err().to_string();
        assert!(err.contains("StandardFrame"));
    }
}
