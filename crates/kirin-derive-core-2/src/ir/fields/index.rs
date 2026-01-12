use quote::{ToTokens, format_ident};

#[derive(Debug, Clone)]
pub struct FieldIndex {
    pub ident: Option<syn::Ident>,
    pub index: usize,
}

impl FieldIndex {
    pub fn new(ident: Option<syn::Ident>, index: usize) -> Self {
        Self { ident, index }
    }

    pub fn name(&self) -> FieldName<'_> {
        FieldName { parent: self }
    }
}

pub struct FieldName<'a> {
    parent: &'a FieldIndex,
}

impl ToTokens for FieldName<'_> {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        if let Some(ident) = &self.parent.ident {
            ident.to_tokens(tokens);
        } else {
            let index = format_ident!("field_{}", self.parent.index);
            tokens.extend(quote::quote! { #index });
        }
    }
}
