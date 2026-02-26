//! Code generator for `#[derive(StageMeta)]`.
//!
//! Unlike the other modules in this crate (`builder`, `field`, `marker`,
//! `property`), this generator does **not** use the `kirin-derive-core` IR
//! system (`Input<StandardLayout>`, `Scan`/`Emit` visitors). Those modules
//! process dialect enum/struct definitions annotated with `#[kirin(...)]` and
//! classify fields into IR categories (arguments, results, regions, etc.).
//!
//! This module instead targets **compile-stage enums** — enums whose variants
//! each wrap a `StageInfo<L>` for some dialect `L`. These enums represent the
//! set of compilation stages in a pipeline and have no IR field categories.
//! The input is parsed directly with `syn` using `#[stage(...)]` attributes:
//!
//! ```ignore
//! #[derive(StageMeta)]
//! #[stage(crate = "kirin_ir")]          // optional crate path override
//! enum MixedStage {
//!     #[stage(name = "parse")]
//!     Parse(StageInfo<FunctionBody>),
//!     #[stage(name = "lower")]
//!     Lower(StageInfo<LowerBody>),
//! }
//! ```
//!
//! The macro generates:
//! - `HasStageInfo<L>` for each unique dialect type (with or-patterns when
//!   multiple variants share the same dialect).
//! - `StageMeta` with stage identity delegation, `from_stage_name()`
//!   dispatch, and the `Languages` associated type for dialect tuple dispatch.

use std::collections::BTreeMap;

use kirin_derive_core::stage::{self, StageVariantInfo};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{DeriveInput, Type};

/// Generate `HasStageInfo<L>` + `StageMeta` impls for a stage enum.
pub fn generate(input: &DeriveInput) -> Result<TokenStream, syn::Error> {
    let variants = stage::parse_stage_variants(input)?;

    let ir_crate_str = stage::parse_ir_crate_path(&input.attrs)?;
    let ir_crate: syn::Path = syn::parse_str(&ir_crate_str)
        .map_err(|e| syn::Error::new_spanned(input, format!("invalid crate path: {e}")))?;

    let enum_ident = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let mut tokens = TokenStream::new();

    // Group variants by dialect type (using the string representation for dedup).
    let mut dialect_groups: BTreeMap<String, Vec<&StageVariantInfo>> = BTreeMap::new();
    for v in &variants {
        let ty = &v.dialect_ty;
        let key = quote!(#ty).to_string();
        dialect_groups.entry(key).or_default().push(v);
    }

    // Deduplicated dialect types (preserving first-seen order for tuple construction).
    let mut seen_dialect_keys: Vec<String> = Vec::new();
    let mut unique_dialects: Vec<&Type> = Vec::new();
    for v in &variants {
        let ty = &v.dialect_ty;
        let key = quote!(#ty).to_string();
        if !seen_dialect_keys.contains(&key) {
            seen_dialect_keys.push(key);
            unique_dialects.push(&v.dialect_ty);
        }
    }

    // 1. HasStageInfo<L> per unique dialect
    for (key, group) in &dialect_groups {
        let dialect_ty = &group[0].dialect_ty;
        let non_group_variants: Vec<&StageVariantInfo> = variants
            .iter()
            .filter(|v| {
                let ty = &v.dialect_ty;
                &quote!(#ty).to_string() != key
            })
            .collect();

        // Or-pattern for matching variants in this group
        let group_idents: Vec<&syn::Ident> = group.iter().map(|v| &v.ident).collect();
        let field = format_ident!("s");

        let try_ref_arms = if non_group_variants.is_empty() {
            // All variants share this dialect — simple match
            quote! {
                #( #enum_ident::#group_idents(#field) )|* => Some(#field),
            }
        } else {
            let non_idents: Vec<&syn::Ident> =
                non_group_variants.iter().map(|v| &v.ident).collect();
            quote! {
                #( #enum_ident::#group_idents(#field) )|* => Some(#field),
                #( #enum_ident::#non_idents(_) )|* => None,
            }
        };

        let try_mut_arms = if non_group_variants.is_empty() {
            quote! {
                #( #enum_ident::#group_idents(#field) )|* => Some(#field),
            }
        } else {
            let non_idents: Vec<&syn::Ident> =
                non_group_variants.iter().map(|v| &v.ident).collect();
            quote! {
                #( #enum_ident::#group_idents(#field) )|* => Some(#field),
                #( #enum_ident::#non_idents(_) )|* => None,
            }
        };

        tokens.extend(quote! {
            #[automatically_derived]
            impl #impl_generics #ir_crate::HasStageInfo<#dialect_ty> for #enum_ident #ty_generics #where_clause {
                fn try_stage_info(&self) -> Option<&#ir_crate::StageInfo<#dialect_ty>> {
                    match self { #try_ref_arms }
                }
                fn try_stage_info_mut(&mut self) -> Option<&mut #ir_crate::StageInfo<#dialect_ty>> {
                    match self { #try_mut_arms }
                }
            }
        });
    }

    // 2. StageMeta impl
    // Build Languages tuple: (A, (B, ()))
    let languages_ty = unique_dialects
        .iter()
        .rev()
        .fold(quote!(()), |acc, dialect| quote!((#dialect, #acc)));

    // stage_name / set_stage_name / stage_id / set_stage_id — delegate to inner StageInfo
    let all_idents: Vec<&syn::Ident> = variants.iter().map(|v| &v.ident).collect();

    let stage_name_arms = quote! {
        #( #enum_ident::#all_idents(s) => #ir_crate::StageMeta::stage_name(s), )*
    };
    let set_stage_name_arms = quote! {
        #( #enum_ident::#all_idents(s) => #ir_crate::StageMeta::set_stage_name(s, name), )*
    };
    let stage_id_arms = quote! {
        #( #enum_ident::#all_idents(s) => #ir_crate::StageMeta::stage_id(s), )*
    };
    let set_stage_id_arms = quote! {
        #( #enum_ident::#all_idents(s) => #ir_crate::StageMeta::set_stage_id(s, id), )*
    };

    // from_stage_name
    let from_name_arms: Vec<TokenStream> = variants
        .iter()
        .map(|v| {
            let name = &v.stage_name;
            let ident = &v.ident;
            let dialect = &v.dialect_ty;
            quote! {
                #name => Ok(#enum_ident::#ident(#ir_crate::StageInfo::<#dialect>::default())),
            }
        })
        .collect();

    // declared_stage_names
    let stage_names: Vec<&str> = variants.iter().map(|v| v.stage_name.as_str()).collect();

    tokens.extend(quote! {
        #[automatically_derived]
        impl #impl_generics #ir_crate::StageMeta for #enum_ident #ty_generics #where_clause {
            type Languages = #languages_ty;

            fn stage_name(&self) -> Option<#ir_crate::GlobalSymbol> {
                match self { #stage_name_arms }
            }

            fn set_stage_name(&mut self, name: Option<#ir_crate::GlobalSymbol>) {
                match self { #set_stage_name_arms }
            }

            fn stage_id(&self) -> Option<#ir_crate::CompileStage> {
                match self { #stage_id_arms }
            }

            fn set_stage_id(&mut self, id: Option<#ir_crate::CompileStage>) {
                match self { #set_stage_id_arms }
            }

            fn from_stage_name(stage_name: &str) -> Result<Self, String> {
                match stage_name {
                    #( #from_name_arms )*
                    _ => Err(format!("no stage variant mapping for '@{}'", stage_name)),
                }
            }

            fn declared_stage_names() -> &'static [&'static str] {
                &[#( #stage_names ),*]
            }
        }
    });

    Ok(tokens)
}
