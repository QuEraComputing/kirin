use quote::quote;
use proc_macro2::TokenStream;

use crate::DeriveHelperAttribute;

pub struct DeriveContext<A> {
    pub input: syn::DeriveInput,
    pub trait_path: TokenStream,
    pub trait_impl: Vec<TokenStream>,
    pub helper_impls: Vec<TokenStream>,
    pub attribute_info: A,
}

impl<A: DeriveHelperAttribute> DeriveContext<A> {
    pub fn new(trait_path: TokenStream, input: syn::DeriveInput) -> Self {
        let attribute_info = A::scan(&input).unwrap();
        Self {
            input,
            trait_path,
            trait_impl: Vec::new(),
            helper_impls: Vec::new(),
            attribute_info,
        }
    }

    pub fn global_wraps(&self) -> bool {
        self.attribute_info.global_wraps()
    }

    pub fn variant_wraps(&self, variant: &syn::Ident) -> bool {
        self.attribute_info.variant_wraps(variant)
    }

    pub fn write_trait_impl(&mut self, code: TokenStream) {
        self.trait_impl.push(code);
    }

    pub fn write_helper_impl(&mut self, code: TokenStream) {
        self.helper_impls.push(code);
    }

    pub fn generate(&self) -> TokenStream {
        let trait_impls = &self.trait_impl;
        let helper_impls = &self.helper_impls;
        let trait_path = &self.trait_path;

        let name = &self.input.ident;
        let (impl_generics, ty_generics, where_clause) = self.input.generics.split_for_impl();
        quote! {
            #[automatically_derived]
            impl #impl_generics #trait_path for #name #ty_generics #where_clause {
                #(#trait_impls)*
            }
            #(#helper_impls)*
        }
    }
}
