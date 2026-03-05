use proc_macro2::Span;

/// Manipulates generics for generated trait impls.
///
/// Adds the `'ir` lifetime and optional `L: Language` type parameter
/// required by Kirin trait impls.
pub struct GenericsBuilder<'a> {
    ir_path: &'a syn::Path,
}

impl<'a> GenericsBuilder<'a> {
    /// Create a builder that uses `ir_path` for trait bounds (e.g., `kirin_ir::Dialect`).
    pub fn new(ir_path: &'a syn::Path) -> Self {
        Self { ir_path }
    }

    /// Add `'tokens` and `'src: 'tokens` lifetimes to `base` if not already present.
    pub fn with_lifetimes(&self, base: &syn::Generics) -> syn::Generics {
        let mut generics = base.clone();

        let tokens_lt = syn::Lifetime::new("'tokens", Span::call_site());
        if !generics
            .params
            .iter()
            .any(|p| matches!(p, syn::GenericParam::Lifetime(l) if l.lifetime.ident == "tokens"))
        {
            generics.params.insert(
                0,
                syn::GenericParam::Lifetime(syn::LifetimeParam::new(tokens_lt.clone())),
            );
        }

        let src_lt = syn::Lifetime::new("'src", Span::call_site());
        if !generics
            .params
            .iter()
            .any(|p| matches!(p, syn::GenericParam::Lifetime(l) if l.lifetime.ident == "src"))
        {
            let mut src_param = syn::LifetimeParam::new(src_lt);
            src_param.bounds.push(tokens_lt);
            generics
                .params
                .insert(1, syn::GenericParam::Lifetime(src_param));
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
