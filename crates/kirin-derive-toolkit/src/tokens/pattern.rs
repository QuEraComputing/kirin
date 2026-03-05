use proc_macro2::TokenStream;
use quote::{ToTokens, quote};

/// Destructuring pattern for struct/enum fields.
///
/// Renders as `{ a, b, c }` for named fields or `(a, b, c)` for tuple fields.
/// Built automatically by [`Statement::field_bindings`](crate::ir::Statement::field_bindings).
#[derive(Clone, Debug)]
pub struct Pattern {
    /// If true, renders as `{ a, b }` (named fields); otherwise `(a, b)` (tuple fields).
    pub named: bool,
    /// Binding names for each field.
    pub names: Vec<TokenStream>,
}

impl Pattern {
    /// Create a new pattern with the given field style and binding names.
    pub fn new(named: bool, names: Vec<TokenStream>) -> Self {
        Self { named, names }
    }

    /// Return true if the pattern has no bindings (unit struct/variant).
    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }
}

impl ToTokens for Pattern {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        if self.names.is_empty() {
            return;
        }
        let names = &self.names;
        if self.named {
            tokens.extend(quote! { { #(#names),* } });
        } else {
            tokens.extend(quote! { ( #(#names),* ) });
        }
    }
}
