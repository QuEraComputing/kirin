//! Stage enum parsing utilities for `StageMeta` derives.
//!
//! Parses `#[stage(...)]` attributes on enum variants to extract stage names
//! and dialect type parameters.

use syn::{DeriveInput, Fields, GenericArgument, PathArguments, Type};

pub const DEFAULT_IR_CRATE: &str = "::kirin::ir";

/// Parsed metadata from a single stage enum variant.
pub struct StageVariantInfo {
    pub ident: syn::Ident,
    pub stage_name: String,
    pub dialect_ty: Type,
}

/// Extracts the `#[stage(crate = "...")]` override from attributes.
pub fn parse_ir_crate_path(attrs: &[syn::Attribute]) -> Result<String, syn::Error> {
    for attr in attrs {
        if !attr.path().is_ident("stage") {
            continue;
        }
        let mut crate_path = None;
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("crate") {
                let value = meta.value()?;
                let lit: syn::LitStr = value.parse()?;
                crate_path = Some(lit.value());
                Ok(())
            } else {
                Ok(())
            }
        })?;
        if let Some(path) = crate_path {
            return Ok(path);
        }
    }
    Ok(DEFAULT_IR_CRATE.to_string())
}

/// Parses a single enum variant's `#[stage(...)]` attributes.
pub fn parse_stage_variant(variant: &syn::Variant) -> Result<StageVariantInfo, syn::Error> {
    let stage_name = parse_stage_name_attr(variant)?;

    let field_ty = match &variant.fields {
        Fields::Unnamed(fields) if fields.unnamed.len() == 1 => &fields.unnamed[0].ty,
        _ => {
            return Err(syn::Error::new_spanned(
                variant,
                "each variant must be a single-field tuple, e.g. `Variant(StageInfo<L>)`",
            ));
        }
    };

    let dialect_ty = extract_stage_info_type_param(field_ty).ok_or_else(|| {
        syn::Error::new_spanned(
            field_ty,
            "field type must be `StageInfo<L>` where L is a dialect type",
        )
    })?;

    Ok(StageVariantInfo {
        ident: variant.ident.clone(),
        stage_name,
        dialect_ty,
    })
}

/// Parses all variants of a stage enum.
pub fn parse_stage_variants(input: &DeriveInput) -> Result<Vec<StageVariantInfo>, syn::Error> {
    let enum_data = match &input.data {
        syn::Data::Enum(data) => data,
        _ => {
            return Err(syn::Error::new_spanned(
                input,
                "stage derive macros can only be applied to enums",
            ));
        }
    };

    let variants: Vec<StageVariantInfo> = enum_data
        .variants
        .iter()
        .map(parse_stage_variant)
        .collect::<Result<_, _>>()?;

    if variants.is_empty() {
        return Err(syn::Error::new_spanned(
            input,
            "stage enum requires at least one variant",
        ));
    }

    Ok(variants)
}

fn parse_stage_name_attr(variant: &syn::Variant) -> Result<String, syn::Error> {
    for attr in &variant.attrs {
        if !attr.path().is_ident("stage") {
            continue;
        }
        let mut name = None;
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("name") {
                let value = meta.value()?;
                let lit: syn::LitStr = value.parse()?;
                name = Some(lit.value());
                Ok(())
            } else {
                Ok(())
            }
        })?;
        if let Some(n) = name {
            return Ok(n);
        }
    }
    Err(syn::Error::new_spanned(
        variant,
        "missing `#[stage(name = \"...\")]` attribute",
    ))
}

fn extract_stage_info_type_param(ty: &Type) -> Option<Type> {
    let path = match ty {
        Type::Path(tp) => &tp.path,
        _ => return None,
    };

    let last_segment = path.segments.last()?;
    if last_segment.ident != "StageInfo" {
        return None;
    }

    match &last_segment.arguments {
        PathArguments::AngleBracketed(args) if args.args.len() == 1 => match &args.args[0] {
            GenericArgument::Type(t) => Some(t.clone()),
            _ => None,
        },
        _ => None,
    }
}
