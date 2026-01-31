use proc_macro2::TokenStream;
use quote::{ToTokens, quote};

pub struct IterStructDefTokens {
    pub name: TokenStream,
    pub generics: TokenStream,
    pub inner_type: TokenStream,
}

pub struct IterStructDefTokensBuilder {
    name: Option<TokenStream>,
    generics: Option<TokenStream>,
    inner_type: Option<TokenStream>,
}

impl IterStructDefTokens {
    pub fn builder() -> IterStructDefTokensBuilder {
        IterStructDefTokensBuilder {
            name: None,
            generics: None,
            inner_type: None,
        }
    }
}

impl IterStructDefTokensBuilder {
    pub fn name(mut self, value: impl ToTokens) -> Self {
        self.name = Some(value.to_token_stream());
        self
    }

    pub fn generics(mut self, value: impl ToTokens) -> Self {
        self.generics = Some(value.to_token_stream());
        self
    }

    pub fn inner_type(mut self, value: impl ToTokens) -> Self {
        self.inner_type = Some(value.to_token_stream());
        self
    }

    pub fn build(self) -> IterStructDefTokens {
        IterStructDefTokens {
            name: self.name.expect("name is required"),
            generics: self.generics.unwrap_or_default(),
            inner_type: self.inner_type.expect("inner_type is required"),
        }
    }
}

impl ToTokens for IterStructDefTokens {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let name = &self.name;
        let generics = &self.generics;
        let inner_type = &self.inner_type;
        tokens.extend(quote! {
            #[automatically_derived]
            pub struct #name #generics {
                inner: #inner_type,
            }
        });
    }
}

pub struct VariantDefTokens {
    pub name: syn::Ident,
    pub inner_type: TokenStream,
}

pub struct VariantDefTokensBuilder {
    name: Option<syn::Ident>,
    inner_type: Option<TokenStream>,
}

impl VariantDefTokens {
    pub fn builder() -> VariantDefTokensBuilder {
        VariantDefTokensBuilder {
            name: None,
            inner_type: None,
        }
    }
}

impl VariantDefTokensBuilder {
    pub fn name(mut self, value: impl Into<syn::Ident>) -> Self {
        self.name = Some(value.into());
        self
    }

    pub fn inner_type(mut self, value: impl ToTokens) -> Self {
        self.inner_type = Some(value.to_token_stream());
        self
    }

    pub fn build(self) -> VariantDefTokens {
        VariantDefTokens {
            name: self.name.expect("name is required"),
            inner_type: self.inner_type.expect("inner_type is required"),
        }
    }
}

impl ToTokens for VariantDefTokens {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let name = &self.name;
        let inner_type = &self.inner_type;
        tokens.extend(quote! { #name(#inner_type) });
    }
}

pub struct IterEnumDefTokens {
    pub name: TokenStream,
    pub generics: TokenStream,
    pub variants: Vec<VariantDefTokens>,
}

pub struct IterEnumDefTokensBuilder {
    name: Option<TokenStream>,
    generics: Option<TokenStream>,
    variants: Vec<VariantDefTokens>,
}

impl IterEnumDefTokens {
    pub fn builder() -> IterEnumDefTokensBuilder {
        IterEnumDefTokensBuilder {
            name: None,
            generics: None,
            variants: Vec::new(),
        }
    }
}

impl IterEnumDefTokensBuilder {
    pub fn name(mut self, value: impl ToTokens) -> Self {
        self.name = Some(value.to_token_stream());
        self
    }

    pub fn generics(mut self, value: impl ToTokens) -> Self {
        self.generics = Some(value.to_token_stream());
        self
    }

    pub fn variants(mut self, value: Vec<VariantDefTokens>) -> Self {
        self.variants = value;
        self
    }

    pub fn push_variant(mut self, value: VariantDefTokens) -> Self {
        self.variants.push(value);
        self
    }

    pub fn build(self) -> IterEnumDefTokens {
        IterEnumDefTokens {
            name: self.name.expect("name is required"),
            generics: self.generics.unwrap_or_default(),
            variants: self.variants,
        }
    }
}

impl Default for IterStructDefTokensBuilder {
    fn default() -> Self {
        IterStructDefTokens::builder()
    }
}

impl Default for VariantDefTokensBuilder {
    fn default() -> Self {
        VariantDefTokens::builder()
    }
}

impl Default for IterEnumDefTokensBuilder {
    fn default() -> Self {
        IterEnumDefTokens::builder()
    }
}

impl ToTokens for IterEnumDefTokens {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let name = &self.name;
        let generics = &self.generics;
        let variants = &self.variants;
        tokens.extend(quote! {
            #[automatically_derived]
            pub enum #name #generics {
                #(#variants),*
            }
        });
    }
}
