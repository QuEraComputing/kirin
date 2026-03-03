use proc_macro2::Span;

/// Builder for generics with common lifetime and type parameters.
///
/// This builder helps construct generic parameters for generated types,
/// particularly for AST types that need `'tokens`, `'src`, and `Language` parameters.
pub struct GenericsBuilder<'a> {
    ir_path: &'a syn::Path,
}

impl<'a> GenericsBuilder<'a> {
    /// Creates a new generics builder.
    ///
    /// The `ir_path` is used for adding bounds like `Language: ir_path::Dialect`.
    pub fn new(ir_path: &'a syn::Path) -> Self {
        Self { ir_path }
    }

    /// Adds `'tokens` and `'src: 'tokens` lifetimes to the generics.
    ///
    /// This is useful for types that work with token streams and source references.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let builder = GenericsBuilder::new(&ir_path);
    /// let generics = builder.with_lifetimes(&base_generics);
    /// // generics now has <'tokens, 'src: 'tokens, ...original params...>
    /// ```
    pub fn with_lifetimes(&self, base: &syn::Generics) -> syn::Generics {
        let mut generics = base.clone();

        // Add 'tokens lifetime at the beginning if not present
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

        // Add 'src: 'tokens lifetime after 'tokens if not present
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

    /// Adds lifetimes and a `Language: Dialect` type parameter.
    ///
    /// This is used for AST types and their trait implementations where
    /// `Language` only needs the `Dialect` bound.
    pub fn with_language(&self, base: &syn::Generics) -> syn::Generics {
        let mut generics = self.with_lifetimes(base);
        let ir_path = self.ir_path;

        // Add Language type parameter if not present
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

    /// Adds lifetimes and a `Language` type parameter without bounds.
    ///
    /// This is used when the `Language: Dialect` bound should be specified
    /// in the where clause instead of on the type parameter.
    pub fn with_language_unbounded(&self, base: &syn::Generics) -> syn::Generics {
        let mut generics = self.with_lifetimes(base);

        // Add Language type parameter without any bounds
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
