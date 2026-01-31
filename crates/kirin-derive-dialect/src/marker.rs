use kirin_derive_core::{
    ir::{self, Layout},
    prelude::*,
    tokens::TraitAssocTypeImplTokens,
};
use proc_macro2::{Span, TokenStream};
use quote::ToTokens;

pub fn derive_marker<L: Layout>(input: &ir::Input<L>, trait_path: &syn::Path) -> TokenStream {
    let type_lattice = &input.attrs.type_lattice;
    TraitAssocTypeImplTokens::builder()
        .generics(&input.generics)
        .trait_path(trait_path)
        .type_name(&input.name)
        .assoc_type_ident(syn::Ident::new("TypeLattice", Span::call_site()))
        .assoc_type(type_lattice)
        .build()
        .to_token_stream()
}

#[cfg(test)]
mod tests {
    use super::*;
    use kirin_derive_core::ir::StandardLayout;

    #[test]
    fn test_marker_struct() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(type_lattice = T, crate = ::my::path)]
            pub struct MyStruct;
        };
        let ir = ir::Input::<StandardLayout>::from_derive_input(&input).unwrap();
        let trait_path = syn::parse_quote!(MarkerTrait);
        let tokens = derive_marker(&ir, &trait_path);
        insta::assert_snapshot!(tokens.to_string());
    }

    #[test]
    fn test_marker_enum() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(type_lattice = T, crate = ::my::path)]
            pub enum MyEnum {
                A,
                B,
            }
        };
        let ir = ir::Input::<StandardLayout>::from_derive_input(&input).unwrap();
        let trait_path = syn::parse_quote!(MarkerTrait);
        let tokens = derive_marker(&ir, &trait_path);
        insta::assert_snapshot!(tokens.to_string());
    }
}
