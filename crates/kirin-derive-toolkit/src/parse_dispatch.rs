//! Code generation for `#[derive(ParseDispatch)]` on stage enums.
//!
//! Generates a monomorphic [`ParseDispatch`] implementation that dispatches to
//! concrete dialect parsers with concrete lifetimes, avoiding the HRTB bounds
//! that cause E0275 with `Block`/`Region`-containing types.
//!
//! Reuses the same `#[stage(...)]` attribute parsing as `StageMeta`.

use crate::stage::{self, StageVariantInfo};
use proc_macro2::TokenStream;
use quote::quote;
use syn::DeriveInput;

/// Default path to the kirin-chumsky crate used when no override is provided.
pub const DEFAULT_CHUMSKY_CRATE: &str = "::kirin::parsers";

/// Extracts the `#[stage(chumsky_crate = "...")]` override from attributes.
fn parse_chumsky_crate_path(attrs: &[syn::Attribute]) -> Result<String, syn::Error> {
    for attr in attrs {
        if !attr.path().is_ident("stage") {
            continue;
        }
        let mut crate_path = None;
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("chumsky_crate") {
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
    Ok(DEFAULT_CHUMSKY_CRATE.to_string())
}

/// Generate a `ParseDispatch` implementation for a stage enum.
///
/// For each unique dialect type `L` among the variants, generates a match arm
/// that calls `first_pass_concrete::<L>` / `second_pass_concrete::<L>`.
pub fn generate(input: &DeriveInput) -> Result<TokenStream, syn::Error> {
    let variants = stage::parse_stage_variants(input)?;

    let ir_crate_str = stage::parse_ir_crate_path(&input.attrs)?;
    let ir_crate: syn::Path = syn::parse_str(&ir_crate_str)
        .map_err(|e| syn::Error::new_spanned(input, format!("invalid IR crate path: {e}")))?;

    let chumsky_crate_str = parse_chumsky_crate_path(&input.attrs)?;
    let chumsky_crate: syn::Path = syn::parse_str(&chumsky_crate_str)
        .map_err(|e| syn::Error::new_spanned(input, format!("invalid chumsky crate path: {e}")))?;

    let enum_ident = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    // Build dispatch arms: for each variant, dispatch to the dialect's concrete helper
    let first_pass_arms = build_dispatch_arms(
        &variants,
        enum_ident,
        &ir_crate,
        &chumsky_crate,
        DispatchPass::First,
    );
    let second_pass_arms = build_dispatch_arms(
        &variants,
        enum_ident,
        &ir_crate,
        &chumsky_crate,
        DispatchPass::Second,
    );

    Ok(quote! {
        #[automatically_derived]
        impl #impl_generics #chumsky_crate::ParseDispatch for #enum_ident #ty_generics #where_clause {
            fn dispatch_first_pass(
                &mut self,
                stage_id: #ir_crate::CompileStage,
                ctx: &mut #chumsky_crate::FirstPassCtx<'_>,
            ) -> ::core::result::Result<
                ::core::option::Option<#chumsky_crate::FirstPassDispatchResult>,
                #chumsky_crate::FunctionParseError,
            > {
                match self {
                    #first_pass_arms
                }
            }

            fn dispatch_second_pass(
                &mut self,
                stage_id: #ir_crate::CompileStage,
                ctx: &mut #chumsky_crate::SecondPassCtx<'_>,
            ) -> ::core::result::Result<
                ::core::option::Option<usize>,
                #chumsky_crate::FunctionParseError,
            > {
                match self {
                    #second_pass_arms
                }
            }
        }
    })
}

enum DispatchPass {
    First,
    Second,
}

fn build_dispatch_arms(
    variants: &[StageVariantInfo],
    enum_ident: &syn::Ident,
    _ir_crate: &syn::Path,
    chumsky_crate: &syn::Path,
    pass: DispatchPass,
) -> TokenStream {
    let mut arms = TokenStream::new();

    for v in variants {
        let ident = &v.ident;
        let dialect_ty = &v.dialect_ty;

        let body = match pass {
            DispatchPass::First => quote! {
                #chumsky_crate::first_pass_concrete::<#dialect_ty>(stage_info, stage_id, ctx)
                    .map(::core::option::Option::Some)
            },
            DispatchPass::Second => quote! {
                #chumsky_crate::second_pass_concrete::<#dialect_ty>(stage_info, stage_id, ctx)
                    .map(::core::option::Option::Some)
            },
        };

        arms.extend(quote! {
            #enum_ident::#ident(stage_info) => { #body }
        });
    }

    arms
}

#[cfg(test)]
mod tests {
    use super::*;

    fn generate_parse_dispatch_code(input: syn::DeriveInput) -> String {
        let tokens = generate(&input).expect("Failed to generate ParseDispatch derive");
        crate::test_util::rustfmt_tokens(&tokens)
    }

    #[test]
    fn test_parse_dispatch_single_dialect() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[stage(crate = "kirin_ir", chumsky_crate = "kirin_chumsky")]
            enum SimpleStage {
                #[stage(name = "arith")]
                Arith(StageInfo<ArithDialect>),
            }
        };
        insta::assert_snapshot!(generate_parse_dispatch_code(input));
    }

    #[test]
    fn test_parse_dispatch_multi_dialect() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[stage(crate = "kirin_ir", chumsky_crate = "kirin_chumsky")]
            enum MixedStage {
                #[stage(name = "A")]
                StageA(StageInfo<FunctionBody>),
                #[stage(name = "B")]
                StageB(StageInfo<LowerBody>),
            }
        };
        insta::assert_snapshot!(generate_parse_dispatch_code(input));
    }

    #[test]
    fn test_parse_dispatch_duplicate_dialect() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[stage(crate = "kirin_ir", chumsky_crate = "kirin_chumsky")]
            enum StageBucket {
                #[stage(name = "A")]
                Parse(StageInfo<FunctionBody>),
                #[stage(name = "B")]
                Lower(StageInfo<FunctionBody>),
            }
        };
        insta::assert_snapshot!(generate_parse_dispatch_code(input));
    }

    #[test]
    fn test_parse_dispatch_default_chumsky_crate() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[stage(crate = "kirin_ir")]
            enum SimpleStage {
                #[stage(name = "arith")]
                Arith(StageInfo<ArithDialect>),
            }
        };
        let tokens = generate(&input).expect("Failed to generate");
        let code = tokens.to_string();
        assert!(
            code.contains(":: kirin :: parsers"),
            "Should use default chumsky crate path, got: {code}"
        );
    }
}
