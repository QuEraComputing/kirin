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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_pattern_is_empty() {
        let p = Pattern::new(false, vec![]);
        assert!(p.is_empty());
    }

    #[test]
    fn non_empty_pattern_is_not_empty() {
        let p = Pattern::new(false, vec![quote! { x }]);
        assert!(!p.is_empty());
    }

    #[test]
    fn empty_pattern_produces_no_tokens() {
        let p = Pattern::new(true, vec![]);
        let tokens = p.to_token_stream();
        assert!(tokens.is_empty());
    }

    #[test]
    fn named_pattern_uses_braces() {
        let p = Pattern::new(true, vec![quote! { a }, quote! { b }]);
        let s = p.to_token_stream().to_string();
        assert!(s.contains('{'), "Expected braces in: {s}");
        assert!(s.contains('}'), "Expected braces in: {s}");
        assert!(s.contains("a"));
        assert!(s.contains("b"));
    }

    #[test]
    fn tuple_pattern_uses_parens() {
        let p = Pattern::new(false, vec![quote! { x }, quote! { y }]);
        let s = p.to_token_stream().to_string();
        assert!(s.contains('('), "Expected parens in: {s}");
        assert!(s.contains(')'), "Expected parens in: {s}");
        assert!(s.contains("x"));
        assert!(s.contains("y"));
    }

    #[test]
    fn single_field_named_pattern() {
        let p = Pattern::new(true, vec![quote! { only }]);
        let s = p.to_token_stream().to_string();
        assert!(s.contains("only"));
        assert!(s.contains('{'));
    }

    #[test]
    fn single_field_tuple_pattern() {
        let p = Pattern::new(false, vec![quote! { only }]);
        let s = p.to_token_stream().to_string();
        assert!(s.contains("only"));
        assert!(s.contains('('));
    }
}
