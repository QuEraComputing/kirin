use super::Builder;
use crate::prelude::*;
use quote::quote;

target! {
    /// Implements the `From` trait for a struct or enum in case
    /// it is a wrapper statement.
    pub struct FromImpl
}

impl<'src> Compile<'src, Fields<'_, 'src, Builder>, FromImpl> for Builder {
    fn compile(&self, node: &Fields<'_, 'src, Builder>) -> FromImpl {
        let Some(wrapper) = node.wrapper() else {
            panic!("FromImpl can only be generated for wrapper statements");
        };

        let name = &node.input().ident;
        let (impl_generics, ty_generics, where_clause) = node.input().generics.split_for_impl();
        let wrapper_type = &wrapper.source().ty;
        let let_name_eq_input: Vec<TokenStream> = node
            .iter()
            .map(|f| {
                let name = f.source_ident();
                if f.is_wrapper() {
                    quote! {let #name = value}
                } else if let Some(expr) = f.attrs().default.as_ref() {
                    quote! {let #name = #expr}
                } else {
                    quote! {let #name = Default::default()}
                }
            })
            .collect();
        let names: Vec<_> = node.iter().map(|f| f.source_ident()).collect();
        let initialize = match node.source() {
            syn::Fields::Named(_) => {
                quote! {{ #(#names),* }}
            }
            syn::Fields::Unnamed(_) => {
                quote! {( #(#names),* )}
            }
            syn::Fields::Unit => {
                quote! {}
            }
        };
        let head = match node.definition() {
            DefinitionStructOrVariant::Struct(_) => {
                quote! { Self }
            }
            DefinitionStructOrVariant::Variant(_) => {
                let name = node.source_ident();
                quote! { Self::#name }
            }
        };

        return quote! {
            impl #impl_generics From<#wrapper_type> for #name #ty_generics #where_clause {
                fn from(value: #wrapper_type) -> Self {
                    #(#let_name_eq_input;)*
                    #head #initialize
                }
            }
        }
        .into();
    }
}
