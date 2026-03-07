//! Stage enum parsing utilities for `StageMeta` derives.
//!
//! Parses `#[stage(...)]` attributes on enum variants to extract stage names
//! and dialect type parameters.

use syn::{DeriveInput, Fields, GenericArgument, PathArguments, Type};

/// Default path to the IR crate used by derive macros when no `#[stage(crate = "...")]` override is present.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_stage_variant_missing_name() {
        let input: DeriveInput = syn::parse_quote! {
            enum Stage {
                Source(StageInfo<HighLevel>),
            }
        };
        let result = parse_stage_variants(&input);
        assert!(result.is_err());
        let err = result.err().unwrap().to_string();
        assert!(
            err.contains("missing"),
            "Expected 'missing' in error: {err}"
        );
    }

    #[test]
    fn test_parse_stage_variant_multi_field() {
        let input: DeriveInput = syn::parse_quote! {
            enum Stage {
                #[stage(name = "source")]
                Source(StageInfo<HighLevel>, u32),
            }
        };
        let result = parse_stage_variants(&input);
        assert!(result.is_err());
        let err = result.err().unwrap().to_string();
        assert!(
            err.contains("single-field tuple"),
            "Expected 'single-field tuple' in error: {err}"
        );
    }

    #[test]
    fn test_parse_stage_variants_on_struct() {
        let input: DeriveInput = syn::parse_quote! {
            struct Stage {
                info: StageInfo<HighLevel>,
            }
        };
        let result = parse_stage_variants(&input);
        assert!(result.is_err());
        let err = result.err().unwrap().to_string();
        assert!(
            err.contains("only be applied to enums"),
            "Expected enum-only error: {err}"
        );
    }

    #[test]
    fn test_parse_stage_variants_empty_enum() {
        let input: DeriveInput = syn::parse_quote! {
            enum Stage {}
        };
        let result = parse_stage_variants(&input);
        assert!(result.is_err());
        let err = result.err().unwrap().to_string();
        assert!(
            err.contains("at least one variant"),
            "Expected at-least-one error: {err}"
        );
    }

    #[test]
    fn test_parse_stage_variant_success() {
        let input: DeriveInput = syn::parse_quote! {
            enum Stage {
                #[stage(name = "source")]
                Source(StageInfo<HighLevel>),
            }
        };
        let variants = parse_stage_variants(&input).unwrap();
        assert_eq!(variants.len(), 1);
        assert_eq!(variants[0].stage_name, "source");
        assert_eq!(variants[0].ident, "Source");
    }

    #[test]
    fn test_parse_ir_crate_path_default() {
        let input: DeriveInput = syn::parse_quote! {
            enum Stage {}
        };
        let path = parse_ir_crate_path(&input.attrs).unwrap();
        assert_eq!(path, DEFAULT_IR_CRATE);
    }

    #[test]
    fn test_parse_ir_crate_path_override() {
        let input: DeriveInput = syn::parse_quote! {
            #[stage(crate = "kirin_ir")]
            enum Stage {}
        };
        let path = parse_ir_crate_path(&input.attrs).unwrap();
        assert_eq!(path, "kirin_ir");
    }
}
