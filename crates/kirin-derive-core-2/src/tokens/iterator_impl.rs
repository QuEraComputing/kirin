use proc_macro2::TokenStream;
use quote::{quote, ToTokens};

pub struct IteratorImplTokens {
    pub impl_generics: TokenStream,
    pub name: TokenStream,
    pub type_generics: TokenStream,
    pub where_clause: TokenStream,
    pub item: TokenStream,
    pub next_body: TokenStream,
}

pub struct IteratorImplTokensBuilder {
    impl_generics: Option<TokenStream>,
    name: Option<TokenStream>,
    type_generics: Option<TokenStream>,
    where_clause: Option<TokenStream>,
    item: Option<TokenStream>,
    next_body: Option<TokenStream>,
}

impl IteratorImplTokens {
    pub fn builder() -> IteratorImplTokensBuilder {
        IteratorImplTokensBuilder {
            impl_generics: None,
            name: None,
            type_generics: None,
            where_clause: None,
            item: None,
            next_body: None,
        }
    }
}

impl IteratorImplTokensBuilder {
    pub fn impl_generics(mut self, value: impl ToTokens) -> Self {
        self.impl_generics = Some(value.to_token_stream());
        self
    }

    pub fn name(mut self, value: impl ToTokens) -> Self {
        self.name = Some(value.to_token_stream());
        self
    }

    pub fn type_generics(mut self, value: impl ToTokens) -> Self {
        self.type_generics = Some(value.to_token_stream());
        self
    }

    pub fn where_clause(mut self, value: impl ToTokens) -> Self {
        self.where_clause = Some(value.to_token_stream());
        self
    }

    pub fn item(mut self, value: impl ToTokens) -> Self {
        self.item = Some(value.to_token_stream());
        self
    }

    pub fn next_body(mut self, value: impl ToTokens) -> Self {
        self.next_body = Some(value.to_token_stream());
        self
    }

    pub fn build(self) -> IteratorImplTokens {
        IteratorImplTokens {
            impl_generics: self.impl_generics.expect("impl_generics is required"),
            name: self.name.expect("name is required"),
            type_generics: self.type_generics.unwrap_or_default(),
            where_clause: self.where_clause.unwrap_or_default(),
            item: self.item.expect("item is required"),
            next_body: self.next_body.expect("next_body is required"),
        }
    }
}

impl Default for IteratorImplTokensBuilder {
    fn default() -> Self {
        IteratorImplTokens::builder()
    }
}

impl ToTokens for IteratorImplTokens {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let impl_generics = &self.impl_generics;
        let name = &self.name;
        let type_generics = &self.type_generics;
        let where_clause = &self.where_clause;
        let item = &self.item;
        let next_body = &self.next_body;
        tokens.extend(quote! {
            #[automatically_derived]
            impl #impl_generics Iterator for #name #type_generics #where_clause {
                type Item = #item;
                fn next(&mut self) -> Option<Self::Item> {
                    #next_body
                }
            }
        });
    }
}
