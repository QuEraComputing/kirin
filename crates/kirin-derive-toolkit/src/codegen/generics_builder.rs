use proc_macro2::Span;

/// Manipulates generics for generated trait impls.
///
/// Adds the `'t` lifetime and optional `L: Language` type parameter
/// required by Kirin trait impls.
pub struct GenericsBuilder<'a> {
    ir_path: &'a syn::Path,
}

impl<'a> GenericsBuilder<'a> {
    /// Create a builder that uses `ir_path` for trait bounds (e.g., `kirin_ir::Dialect`).
    pub fn new(ir_path: &'a syn::Path) -> Self {
        Self { ir_path }
    }

    /// Add `'t` lifetime to `base` if not already present.
    pub fn with_lifetimes(&self, base: &syn::Generics) -> syn::Generics {
        let mut generics = base.clone();

        let t_lt = syn::Lifetime::new("'t", Span::call_site());
        if !generics
            .params
            .iter()
            .any(|p| matches!(p, syn::GenericParam::Lifetime(l) if l.lifetime.ident == "t"))
        {
            generics.params.insert(
                0,
                syn::GenericParam::Lifetime(syn::LifetimeParam::new(t_lt)),
            );
        }

        generics
    }

    /// Add lifetimes and a `Language: Dialect` type parameter to `base`.
    pub fn with_language(&self, base: &syn::Generics) -> syn::Generics {
        let mut generics = self.with_lifetimes(base);
        let ir_path = self.ir_path;

        let lang_ident = syn::Ident::new("Language", Span::call_site());
        if !generics
            .params
            .iter()
            .any(|p| matches!(p, syn::GenericParam::Type(t) if t.ident == lang_ident))
        {
            let mut lang_param = syn::TypeParam::from(lang_ident);
            lang_param.bounds.push(syn::parse_quote!(#ir_path::Dialect));
            generics.params.push(syn::GenericParam::Type(lang_param));
        }

        generics
    }

    /// Add lifetimes and a bare `Language` type parameter (no trait bound).
    pub fn with_language_unbounded(&self, base: &syn::Generics) -> syn::Generics {
        let mut generics = self.with_lifetimes(base);

        let lang_ident = syn::Ident::new("Language", Span::call_site());
        if !generics
            .params
            .iter()
            .any(|p| matches!(p, syn::GenericParam::Type(t) if t.ident == lang_ident))
        {
            let lang_param = syn::TypeParam::from(lang_ident);
            generics.params.push(syn::GenericParam::Type(lang_param));
        }

        generics
    }
}
