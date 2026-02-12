extern crate proc_macro;

use kirin_derive_core::stage;
use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{DeriveInput, parse_macro_input};

const DEFAULT_PRETTY_CRATE: &str = "::kirin::pretty";

#[proc_macro_derive(RenderStage, attributes(stage))]
pub fn derive_render_stage(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    match generate(&ast) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.into_compile_error().into(),
    }
}

fn generate(input: &DeriveInput) -> Result<TokenStream2, syn::Error> {
    let variants = stage::parse_stage_variants(input)?;

    let ir_crate_str = stage::parse_ir_crate_path(&input.attrs)?;
    let ir_crate: syn::Path = syn::parse_str(&ir_crate_str)
        .map_err(|e| syn::Error::new_spanned(input, format!("invalid crate path: {e}")))?;

    let pretty_crate_str = parse_pretty_crate_path(&input.attrs)?;
    let pretty_crate: syn::Path = syn::parse_str(&pretty_crate_str)
        .map_err(|e| syn::Error::new_spanned(input, format!("invalid pretty path: {e}")))?;

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

/// Parse the optional `#[stage(pretty = "...")]` attribute on the enum.
fn parse_pretty_crate_path(attrs: &[syn::Attribute]) -> Result<String, syn::Error> {
    for attr in attrs {
        if !attr.path().is_ident("stage") {
            continue;
        }
        let mut pretty_path = None;
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("pretty") {
                let value = meta.value()?;
                let lit: syn::LitStr = value.parse()?;
                pretty_path = Some(lit.value());
                Ok(())
            } else {
                // Ignore unknown keys — other derives may use them.
                Ok(())
            }
        })?;
        if let Some(path) = pretty_path {
            return Ok(path);
        }
    }
    Ok(DEFAULT_PRETTY_CRATE.to_string())
}
