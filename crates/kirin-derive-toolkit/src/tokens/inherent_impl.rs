use proc_macro2::TokenStream;
use quote::{ToTokens, quote};

use super::Method;

/// Generated `impl Type { ... }` block (no trait).
pub struct InherentImpl {
    /// Impl-level generic parameters.
    pub generics: syn::Generics,
    /// The type being implemented.
    pub type_name: TokenStream,
    /// Generic arguments on the type (e.g., `<T>`).
    pub type_generics: TokenStream,
    /// Optional where clause.
    pub where_clause: Option<syn::WhereClause>,
    /// Methods in the impl block.
    pub items: Vec<Method>,
}

impl ToTokens for InherentImpl {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let (impl_generics, _, _) = self.generics.split_for_impl();
        let type_name = &self.type_name;
        let type_generics = &self.type_generics;
        let where_clause = &self.where_clause;
        let items = &self.items;

        tokens.extend(quote! {
            #[automatically_derived]
            impl #impl_generics #type_name #type_generics #where_clause {
                #(#items)*
            }
        });
    }
}
