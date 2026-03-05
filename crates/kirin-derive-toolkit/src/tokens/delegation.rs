use proc_macro2::TokenStream;
use quote::{ToTokens, quote};

pub struct DelegationCall {
    pub wrapper_ty: TokenStream,
    pub trait_path: TokenStream,
    pub trait_method: syn::Ident,
    pub field: TokenStream,
}

impl ToTokens for DelegationCall {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let wrapper_ty = &self.wrapper_ty;
        let trait_path = &self.trait_path;
        let trait_method = &self.trait_method;
        let field = &self.field;
        tokens.extend(quote! { <#wrapper_ty as #trait_path>::#trait_method(#field) });
    }
}

pub struct DelegationAssocType {
    pub wrapper_ty: TokenStream,
    pub trait_path: TokenStream,
    pub trait_generics: TokenStream,
    pub assoc_type_ident: syn::Ident,
}

impl ToTokens for DelegationAssocType {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let wrapper_ty = &self.wrapper_ty;
        let trait_path = &self.trait_path;
        let trait_generics = &self.trait_generics;
        let assoc_type_ident = &self.assoc_type_ident;
        tokens.extend(
            quote! { <#wrapper_ty as #trait_path #trait_generics>::#assoc_type_ident },
        );
    }
}
