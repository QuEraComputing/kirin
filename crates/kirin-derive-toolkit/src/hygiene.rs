use proc_macro2::Span;
use quote::format_ident;

/// Generates prefixed identifiers for derive macro output to avoid
/// name collisions with user code.
pub struct Hygiene {
    prefix: String,
}

impl Hygiene {
    pub fn new(prefix: &str) -> Self {
        Self {
            prefix: prefix.to_string(),
        }
    }

    /// Generate a snake_case identifier: `__{prefix}_{name}`
    pub fn ident(&self, name: &str) -> syn::Ident {
        format_ident!("__{}_{}", self.prefix, name)
    }

    /// Generate a lifetime: `'__{prefix}_{name}`
    pub fn lifetime(&self, name: &str) -> syn::Lifetime {
        syn::Lifetime::new(
            &format!("'__{}_{}", self.prefix, name),
            Span::call_site(),
        )
    }

    /// Generate a CamelCase type identifier: `__{Prefix}{Name}`
    pub fn type_ident(&self, name: &str) -> syn::Ident {
        let camel_prefix = crate::misc::to_camel_case(self.prefix.clone());
        let camel_name = crate::misc::to_camel_case(name.to_string());
        format_ident!("__{}{}", camel_prefix, camel_name)
    }
}
