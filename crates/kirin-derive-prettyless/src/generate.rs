use kirin_derive_toolkit::stage;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::DeriveInput;

const DEFAULT_PRETTY_CRATE: &str = "::kirin::pretty";

pub(crate) fn generate(input: &DeriveInput) -> Result<TokenStream2, syn::Error> {
    let variants = stage::parse_stage_variants(input)?;

    let ir_crate_str = stage::parse_ir_crate_path(&input.attrs)?;
    let ir_crate: syn::Path = syn::parse_str(&ir_crate_str)
        .map_err(|e| syn::Error::new_spanned(input, format!("invalid crate path: {e}")))?;

    let pretty_crate = parse_pretty_crate_path(&input.attrs)?;

    let enum_ident = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let all_idents: Vec<&syn::Ident> = variants.iter().map(|v| &v.ident).collect();

    Ok(quote! {
        impl #impl_generics #pretty_crate::RenderStage for #enum_ident #ty_generics #where_clause {
            fn render_staged_function(
                &self,
                sf: #ir_crate::StagedFunction,
                config: &#pretty_crate::Config,
                global_symbols: &#ir_crate::InternTable<String, #ir_crate::GlobalSymbol>,
            ) -> Result<Option<String>, std::fmt::Error> {
                match self {
                    #( #enum_ident::#all_idents(s) =>
                        #pretty_crate::RenderStage::render_staged_function(s, sf, config, global_symbols), )*
                }
            }
        }
    })
}

/// Parse the optional `#[pretty(crate = ...)]` attribute on the enum.
///
/// Accepts bare path syntax: `#[pretty(crate = kirin_prettyless)]`.
/// Falls back to `::kirin::pretty` if not specified.
fn parse_pretty_crate_path(attrs: &[syn::Attribute]) -> Result<syn::Path, syn::Error> {
    for attr in attrs {
        if !attr.path().is_ident("pretty") {
            continue;
        }
        let mut crate_path = None;
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("crate") {
                let value = meta.value()?;
                let path: syn::Path = value.parse()?;
                crate_path = Some(path);
                Ok(())
            } else {
                Err(meta.error("unsupported attribute"))
            }
        })?;
        if let Some(path) = crate_path {
            return Ok(path);
        }
    }
    Ok(syn::parse_str(DEFAULT_PRETTY_CRATE).unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;
    use kirin_test_utils::rustfmt;

    fn generate_render_stage_code(input: syn::DeriveInput) -> String {
        let tokens = generate(&input).expect("Failed to generate RenderStage");
        rustfmt(tokens.to_string())
    }

    #[test]
    fn test_render_stage_two_variants() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[stage(crate = "kirin_ir")]
            enum Stage {
                #[stage(name = "source")]
                Source(StageInfo<HighLevel>),
                #[stage(name = "lowered")]
                Lowered(StageInfo<LowLevel>),
            }
        };
        insta::assert_snapshot!(generate_render_stage_code(input));
    }

    #[test]
    fn test_render_stage_custom_crate_path() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[stage(crate = "kirin_ir")]
            #[pretty(crate = kirin_prettyless)]
            enum Stage {
                #[stage(name = "source")]
                Source(StageInfo<HighLevel>),
            }
        };
        insta::assert_snapshot!(generate_render_stage_code(input));
    }

    #[test]
    fn test_render_stage_single_variant() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[stage(crate = "kirin_ir")]
            enum SimpleStage {
                #[stage(name = "main")]
                Main(StageInfo<MainDialect>),
            }
        };
        insta::assert_snapshot!(generate_render_stage_code(input));
    }
}
