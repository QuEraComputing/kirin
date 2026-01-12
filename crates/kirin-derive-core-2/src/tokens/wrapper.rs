use proc_macro2::TokenStream;
use quote::{quote, ToTokens};

pub struct WrapperCallTokens {
    pub wrapper_ty: TokenStream,
    pub trait_path: TokenStream,
    pub trait_method: syn::Ident,
    pub field: TokenStream,
}

impl ToTokens for WrapperCallTokens {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let wrapper_ty = &self.wrapper_ty;
        let trait_path = &self.trait_path;
        let trait_method = &self.trait_method;
        let field = &self.field;
        tokens.extend(quote! { <#wrapper_ty as #trait_path>::#trait_method(#field) });
    }
}

pub struct WrapperCallTokensBuilder {
    wrapper_ty: Option<TokenStream>,
    trait_path: Option<TokenStream>,
    trait_method: Option<syn::Ident>,
    field: Option<TokenStream>,
}

impl WrapperCallTokens {
    pub fn builder() -> WrapperCallTokensBuilder {
        WrapperCallTokensBuilder {
            wrapper_ty: None,
            trait_path: None,
            trait_method: None,
            field: None,
        }
    }
}

impl WrapperCallTokensBuilder {
    pub fn wrapper_ty(mut self, value: impl ToTokens) -> Self {
        self.wrapper_ty = Some(value.to_token_stream());
        self
    }

    pub fn trait_path(mut self, value: impl ToTokens) -> Self {
        self.trait_path = Some(value.to_token_stream());
        self
    }

    pub fn trait_method(mut self, value: impl Into<syn::Ident>) -> Self {
        self.trait_method = Some(value.into());
        self
    }

    pub fn field(mut self, value: impl ToTokens) -> Self {
        self.field = Some(value.to_token_stream());
        self
    }

    pub fn build(self) -> WrapperCallTokens {
        WrapperCallTokens {
            wrapper_ty: self.wrapper_ty.expect("wrapper_ty is required"),
            trait_path: self.trait_path.expect("trait_path is required"),
            trait_method: self.trait_method.expect("trait_method is required"),
            field: self.field.expect("field is required"),
        }
    }
}

impl Default for WrapperCallTokensBuilder {
    fn default() -> Self {
        WrapperCallTokens::builder()
    }
}

pub struct WrapperIterTypeTokens {
    pub wrapper_ty: TokenStream,
    pub trait_path: TokenStream,
    pub trait_generics: TokenStream,
    pub assoc_type_ident: syn::Ident,
}

pub struct WrapperIterTypeTokensBuilder {
    wrapper_ty: Option<TokenStream>,
    trait_path: Option<TokenStream>,
    trait_generics: Option<TokenStream>,
    assoc_type_ident: Option<syn::Ident>,
}

impl WrapperIterTypeTokens {
    pub fn builder() -> WrapperIterTypeTokensBuilder {
        WrapperIterTypeTokensBuilder {
            wrapper_ty: None,
            trait_path: None,
            trait_generics: None,
            assoc_type_ident: None,
        }
    }
}

impl WrapperIterTypeTokensBuilder {
    pub fn wrapper_ty(mut self, value: impl ToTokens) -> Self {
        self.wrapper_ty = Some(value.to_token_stream());
        self
    }

    pub fn trait_path(mut self, value: impl ToTokens) -> Self {
        self.trait_path = Some(value.to_token_stream());
        self
    }

    pub fn trait_generics(mut self, value: impl ToTokens) -> Self {
        self.trait_generics = Some(value.to_token_stream());
        self
    }

    pub fn assoc_type_ident(mut self, value: impl Into<syn::Ident>) -> Self {
        self.assoc_type_ident = Some(value.into());
        self
    }

    pub fn build(self) -> WrapperIterTypeTokens {
        WrapperIterTypeTokens {
            wrapper_ty: self.wrapper_ty.expect("wrapper_ty is required"),
            trait_path: self.trait_path.expect("trait_path is required"),
            trait_generics: self.trait_generics.unwrap_or_default(),
            assoc_type_ident: self.assoc_type_ident.expect("assoc_type_ident is required"),
        }
    }
}

impl Default for WrapperIterTypeTokensBuilder {
    fn default() -> Self {
        WrapperIterTypeTokens::builder()
    }
}

impl ToTokens for WrapperIterTypeTokens {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let wrapper_ty = &self.wrapper_ty;
        let trait_path = &self.trait_path;
        let trait_generics = &self.trait_generics;
        let assoc_type_ident = &self.assoc_type_ident;
        tokens.extend(quote! { <#wrapper_ty as #trait_path #trait_generics>::#assoc_type_ident });
    }
}
