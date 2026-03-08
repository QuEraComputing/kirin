use quote::{ToTokens, format_ident};

/// Positional identity of a field — either named (`foo`) or positional (`0`).
#[derive(Debug, Clone)]
pub struct FieldIndex {
    /// Field name, or `None` for tuple/positional fields.
    pub ident: Option<syn::Ident>,
    /// Zero-based position among sibling fields.
    pub index: usize,
}

impl FieldIndex {
    /// Create a field index from an optional name and positional index.
    pub fn new(ident: Option<syn::Ident>, index: usize) -> Self {
        Self { ident, index }
    }

    /// Return a [`ToTokens`](quote::ToTokens)-compatible reference for code generation.
    pub fn name(&self) -> FieldName<'_> {
        FieldName { parent: self }
    }
}

/// Display-ready field reference: named fields emit their ident, positional
/// fields emit their index.
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

#[cfg(test)]
mod tests {
    use super::*;
    use proc_macro2::Span;

    #[test]
    fn field_index_named_to_tokens() {
        let idx = FieldIndex::new(Some(syn::Ident::new("foo", Span::call_site())), 0);
        let name = idx.name();
        let tokens = quote::quote! { #name };
        assert_eq!(tokens.to_string(), "foo");
    }

    #[test]
    fn field_index_positional_to_tokens() {
        let idx = FieldIndex::new(None, 2);
        let name = idx.name();
        let tokens = quote::quote! { #name };
        assert_eq!(tokens.to_string(), "field_2");
    }

    #[test]
    fn field_index_positional_zero() {
        let idx = FieldIndex::new(None, 0);
        let name = idx.name();
        let tokens = quote::quote! { #name };
        assert_eq!(tokens.to_string(), "field_0");
    }
}
