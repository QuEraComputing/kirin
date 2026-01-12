use proc_macro2::TokenStream;
use quote::{quote, ToTokens};

use super::to_stream;

pub struct TraitImplTokens {
    pub impl_generics: TokenStream,
    pub trait_path: TokenStream,
    pub trait_generics: TokenStream,
    pub type_name: TokenStream,
    pub type_generics: TokenStream,
    pub where_clause: TokenStream,
    pub assoc_type_ident: syn::Ident,
    pub assoc_type: TokenStream,
    pub method_name: syn::Ident,
    pub self_arg: TokenStream,
    pub body: TokenStream,
}

struct GenericsTokens {
    impl_generics: TokenStream,
    type_generics: TokenStream,
    where_clause: TokenStream,
}

pub struct TraitImplTokensBuilder {
    generics: Option<GenericsTokens>,
    trait_path: Option<TokenStream>,
    trait_generics: Option<TokenStream>,
    type_name: Option<TokenStream>,
    assoc_type_ident: Option<syn::Ident>,
    assoc_type: Option<TokenStream>,
    method_name: Option<syn::Ident>,
    self_arg: Option<TokenStream>,
    body: Option<TokenStream>,
}

impl TraitImplTokens {
    pub fn builder() -> TraitImplTokensBuilder {
        TraitImplTokensBuilder {
            generics: None,
            trait_path: None,
            trait_generics: None,
            type_name: None,
            assoc_type_ident: None,
            assoc_type: None,
            method_name: None,
            self_arg: None,
            body: None,
        }
    }
}

impl TraitImplTokensBuilder {
    pub fn generics(mut self, generics: &syn::Generics) -> Self {
        let (impl_generics, type_generics, where_clause) = generics.split_for_impl();
        self.generics = Some(GenericsTokens {
            impl_generics: to_stream(impl_generics),
            type_generics: to_stream(type_generics),
            where_clause: to_stream(where_clause),
        });
        self
    }

    pub fn impl_and_type_generics(
        mut self,
        impl_generics: &syn::Generics,
        type_generics: &syn::Generics,
    ) -> Self {
        let (impl_generics, _, where_clause) = impl_generics.split_for_impl();
        let (_, type_generics, _) = type_generics.split_for_impl();
        self.generics = Some(GenericsTokens {
            impl_generics: to_stream(impl_generics),
            type_generics: to_stream(type_generics),
            where_clause: to_stream(where_clause),
        });
        self
    }

    pub fn trait_path(mut self, value: impl ToTokens) -> Self {
        self.trait_path = Some(to_stream(value));
        self
    }

    pub fn trait_generics(mut self, value: impl ToTokens) -> Self {
        self.trait_generics = Some(to_stream(value));
        self
    }

    pub fn type_name(mut self, value: impl ToTokens) -> Self {
        self.type_name = Some(to_stream(value));
        self
    }

    pub fn assoc_type_ident(mut self, value: impl Into<syn::Ident>) -> Self {
        self.assoc_type_ident = Some(value.into());
        self
    }

    pub fn assoc_type(mut self, value: impl ToTokens) -> Self {
        self.assoc_type = Some(to_stream(value));
        self
    }

    pub fn method_name(mut self, value: impl Into<syn::Ident>) -> Self {
        self.method_name = Some(value.into());
        self
    }

    pub fn self_arg(mut self, value: impl ToTokens) -> Self {
        self.self_arg = Some(to_stream(value));
        self
    }

    pub fn body(mut self, value: impl ToTokens) -> Self {
        self.body = Some(to_stream(value));
        self
    }

    pub fn build(self) -> TraitImplTokens {
        let generics = self.generics.expect("generics is required");
        TraitImplTokens {
            impl_generics: generics.impl_generics,
            trait_path: self.trait_path.expect("trait_path is required"),
            trait_generics: self.trait_generics.unwrap_or_default(),
            type_name: self.type_name.expect("type_name is required"),
            type_generics: generics.type_generics,
            where_clause: generics.where_clause,
            assoc_type_ident: self.assoc_type_ident.expect("assoc_type_ident is required"),
            assoc_type: self.assoc_type.expect("assoc_type is required"),
            method_name: self.method_name.expect("method_name is required"),
            self_arg: self.self_arg.expect("self_arg is required"),
            body: self.body.expect("body is required"),
        }
    }
}

impl Default for TraitImplTokensBuilder {
    fn default() -> Self {
        TraitImplTokens::builder()
    }
}

impl ToTokens for TraitImplTokens {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let impl_generics = &self.impl_generics;
        let trait_path = &self.trait_path;
        let trait_generics = &self.trait_generics;
        let type_name = &self.type_name;
        let type_generics = &self.type_generics;
        let where_clause = &self.where_clause;
        let assoc_type_ident = &self.assoc_type_ident;
        let assoc_type = &self.assoc_type;
        let method_name = &self.method_name;
        let self_arg = &self.self_arg;
        let body = &self.body;

        tokens.extend(quote! {
            #[automatically_derived]
            impl #impl_generics #trait_path #trait_generics for #type_name #type_generics #where_clause {
                type #assoc_type_ident = #assoc_type;
                fn #method_name(#self_arg) -> Self::#assoc_type_ident {
                    #body
                }
            }
        });
    }
}

pub struct TraitMethodImplTokens {
    pub impl_generics: TokenStream,
    pub trait_path: TokenStream,
    pub trait_generics: TokenStream,
    pub type_name: TokenStream,
    pub type_generics: TokenStream,
    pub where_clause: TokenStream,
    pub method_name: syn::Ident,
    pub self_arg: TokenStream,
    pub output_type: TokenStream,
    pub body: TokenStream,
}

pub struct TraitMethodImplTokensBuilder {
    generics: Option<GenericsTokens>,
    trait_path: Option<TokenStream>,
    trait_generics: Option<TokenStream>,
    type_name: Option<TokenStream>,
    method_name: Option<syn::Ident>,
    self_arg: Option<TokenStream>,
    output_type: Option<TokenStream>,
    body: Option<TokenStream>,
}

impl TraitMethodImplTokens {
    pub fn builder() -> TraitMethodImplTokensBuilder {
        TraitMethodImplTokensBuilder {
            generics: None,
            trait_path: None,
            trait_generics: None,
            type_name: None,
            method_name: None,
            self_arg: None,
            output_type: None,
            body: None,
        }
    }
}

impl TraitMethodImplTokensBuilder {
    pub fn generics(mut self, generics: &syn::Generics) -> Self {
        let (impl_generics, type_generics, where_clause) = generics.split_for_impl();
        self.generics = Some(GenericsTokens {
            impl_generics: to_stream(impl_generics),
            type_generics: to_stream(type_generics),
            where_clause: to_stream(where_clause),
        });
        self
    }

    pub fn impl_and_type_generics(
        mut self,
        impl_generics: &syn::Generics,
        type_generics: &syn::Generics,
    ) -> Self {
        let (impl_generics, _, where_clause) = impl_generics.split_for_impl();
        let (_, type_generics, _) = type_generics.split_for_impl();
        self.generics = Some(GenericsTokens {
            impl_generics: to_stream(impl_generics),
            type_generics: to_stream(type_generics),
            where_clause: to_stream(where_clause),
        });
        self
    }

    pub fn trait_path(mut self, value: impl ToTokens) -> Self {
        self.trait_path = Some(to_stream(value));
        self
    }

    pub fn trait_generics(mut self, value: impl ToTokens) -> Self {
        self.trait_generics = Some(to_stream(value));
        self
    }

    pub fn type_name(mut self, value: impl ToTokens) -> Self {
        self.type_name = Some(to_stream(value));
        self
    }

    pub fn method_name(mut self, value: impl Into<syn::Ident>) -> Self {
        self.method_name = Some(value.into());
        self
    }

    pub fn self_arg(mut self, value: impl ToTokens) -> Self {
        self.self_arg = Some(to_stream(value));
        self
    }

    pub fn output_type(mut self, value: impl ToTokens) -> Self {
        self.output_type = Some(to_stream(value));
        self
    }

    pub fn body(mut self, value: impl ToTokens) -> Self {
        self.body = Some(to_stream(value));
        self
    }

    pub fn build(self) -> TraitMethodImplTokens {
        let generics = self.generics.expect("generics is required");
        TraitMethodImplTokens {
            impl_generics: generics.impl_generics,
            trait_path: self.trait_path.expect("trait_path is required"),
            trait_generics: self.trait_generics.unwrap_or_default(),
            type_name: self.type_name.expect("type_name is required"),
            type_generics: generics.type_generics,
            where_clause: generics.where_clause,
            method_name: self.method_name.expect("method_name is required"),
            self_arg: self.self_arg.expect("self_arg is required"),
            output_type: self.output_type.expect("output_type is required"),
            body: self.body.expect("body is required"),
        }
    }
}

impl Default for TraitMethodImplTokensBuilder {
    fn default() -> Self {
        TraitMethodImplTokens::builder()
    }
}

impl ToTokens for TraitMethodImplTokens {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let impl_generics = &self.impl_generics;
        let trait_path = &self.trait_path;
        let trait_generics = &self.trait_generics;
        let type_name = &self.type_name;
        let type_generics = &self.type_generics;
        let where_clause = &self.where_clause;
        let method_name = &self.method_name;
        let self_arg = &self.self_arg;
        let output_type = &self.output_type;
        let body = &self.body;

        tokens.extend(quote! {
            #[automatically_derived]
            impl #impl_generics #trait_path #trait_generics for #type_name #type_generics #where_clause {
                fn #method_name(#self_arg) -> #output_type {
                    #body
                }
            }
        });
    }
}

pub struct TraitAssocTypeImplTokens {
    pub impl_generics: TokenStream,
    pub trait_path: TokenStream,
    pub trait_generics: TokenStream,
    pub type_name: TokenStream,
    pub type_generics: TokenStream,
    pub where_clause: TokenStream,
    pub assoc_type_ident: syn::Ident,
    pub assoc_type: TokenStream,
}

pub struct TraitAssocTypeImplTokensBuilder {
    generics: Option<GenericsTokens>,
    trait_path: Option<TokenStream>,
    trait_generics: Option<TokenStream>,
    type_name: Option<TokenStream>,
    assoc_type_ident: Option<syn::Ident>,
    assoc_type: Option<TokenStream>,
}

impl TraitAssocTypeImplTokens {
    pub fn builder() -> TraitAssocTypeImplTokensBuilder {
        TraitAssocTypeImplTokensBuilder {
            generics: None,
            trait_path: None,
            trait_generics: None,
            type_name: None,
            assoc_type_ident: None,
            assoc_type: None,
        }
    }
}

impl TraitAssocTypeImplTokensBuilder {
    pub fn generics(mut self, generics: &syn::Generics) -> Self {
        let (impl_generics, type_generics, where_clause) = generics.split_for_impl();
        self.generics = Some(GenericsTokens {
            impl_generics: to_stream(impl_generics),
            type_generics: to_stream(type_generics),
            where_clause: to_stream(where_clause),
        });
        self
    }

    pub fn impl_and_type_generics(
        mut self,
        impl_generics: &syn::Generics,
        type_generics: &syn::Generics,
    ) -> Self {
        let (impl_generics, _, where_clause) = impl_generics.split_for_impl();
        let (_, type_generics, _) = type_generics.split_for_impl();
        self.generics = Some(GenericsTokens {
            impl_generics: to_stream(impl_generics),
            type_generics: to_stream(type_generics),
            where_clause: to_stream(where_clause),
        });
        self
    }

    pub fn trait_path(mut self, value: impl ToTokens) -> Self {
        self.trait_path = Some(to_stream(value));
        self
    }

    pub fn trait_generics(mut self, value: impl ToTokens) -> Self {
        self.trait_generics = Some(to_stream(value));
        self
    }

    pub fn type_name(mut self, value: impl ToTokens) -> Self {
        self.type_name = Some(to_stream(value));
        self
    }

    pub fn assoc_type_ident(mut self, value: impl Into<syn::Ident>) -> Self {
        self.assoc_type_ident = Some(value.into());
        self
    }

    pub fn assoc_type(mut self, value: impl ToTokens) -> Self {
        self.assoc_type = Some(to_stream(value));
        self
    }

    pub fn build(self) -> TraitAssocTypeImplTokens {
        let generics = self.generics.expect("generics is required");
        TraitAssocTypeImplTokens {
            impl_generics: generics.impl_generics,
            trait_path: self.trait_path.expect("trait_path is required"),
            trait_generics: self.trait_generics.unwrap_or_default(),
            type_name: self.type_name.expect("type_name is required"),
            type_generics: generics.type_generics,
            where_clause: generics.where_clause,
            assoc_type_ident: self.assoc_type_ident.expect("assoc_type_ident is required"),
            assoc_type: self.assoc_type.expect("assoc_type is required"),
        }
    }
}

impl Default for TraitAssocTypeImplTokensBuilder {
    fn default() -> Self {
        TraitAssocTypeImplTokens::builder()
    }
}

impl ToTokens for TraitAssocTypeImplTokens {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let impl_generics = &self.impl_generics;
        let trait_path = &self.trait_path;
        let trait_generics = &self.trait_generics;
        let type_name = &self.type_name;
        let type_generics = &self.type_generics;
        let where_clause = &self.where_clause;
        let assoc_type_ident = &self.assoc_type_ident;
        let assoc_type = &self.assoc_type;

        tokens.extend(quote! {
            #[automatically_derived]
            impl #impl_generics #trait_path #trait_generics for #type_name #type_generics #where_clause {
                type #assoc_type_ident = #assoc_type;
            }
        });
    }
}