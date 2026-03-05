use proc_macro2::TokenStream;
use quote::{ToTokens, quote};

/// Builder for `impl Trait for Type { ... }` blocks.
///
/// ```ignore
/// let imp = TraitImpl::new(generics, quote!(MyTrait), quote!(MyType))
///     .trait_generics(quote!(<'ir, L>))
///     .method(Method {
///         name: format_ident!("my_method"),
///         self_arg: quote!(&self),
///         params: vec![quote!(x: u32)],
///         return_type: Some(quote!(bool)),
///         body: quote! { x > 0 },
///     })
///     .assoc_type(format_ident!("Output"), quote!(u32));
/// // imp implements ToTokens
/// ```
pub struct TraitImpl {
    pub generics: syn::Generics,
    pub trait_path: TokenStream,
    pub trait_generics: TokenStream,
    pub type_name: TokenStream,
    pub type_generics: TokenStream,
    pub where_clause: Option<syn::WhereClause>,
    pub items: Vec<ImplItem>,
}

/// An item inside a trait impl block.
pub enum ImplItem {
    /// A method definition.
    Method(Method),
    /// An associated type definition.
    AssocType(AssocType),
    /// An associated constant definition.
    AssocConst(AssocConst),
}

/// A method definition inside a trait impl.
pub struct Method {
    /// Method name.
    pub name: syn::Ident,
    /// Self receiver (e.g., `&self`, `&mut self`).
    pub self_arg: TokenStream,
    /// Additional parameters after self.
    pub params: Vec<TokenStream>,
    /// Return type (omitted if `None`).
    pub return_type: Option<TokenStream>,
    /// Method body tokens.
    pub body: TokenStream,
}

/// An associated type definition (`type Name = Ty;`).
pub struct AssocType {
    /// The associated type name.
    pub name: syn::Ident,
    /// The concrete type it is set to.
    pub ty: TokenStream,
}

/// An associated constant definition (`const NAME: Ty = val;`).
pub struct AssocConst {
    /// The constant name.
    pub name: syn::Ident,
    /// The constant type.
    pub ty: TokenStream,
    /// The constant value expression.
    pub value: TokenStream,
}

impl TraitImpl {
    pub fn new(
        generics: syn::Generics,
        trait_path: impl ToTokens,
        type_name: impl ToTokens,
    ) -> Self {
        let (_, type_generics, where_clause) = generics.split_for_impl();
        Self {
            type_generics: type_generics.to_token_stream(),
            where_clause: where_clause.cloned(),
            generics,
            trait_path: trait_path.to_token_stream(),
            trait_generics: TokenStream::new(),
            type_name: type_name.to_token_stream(),
            items: Vec::new(),
        }
    }

    pub fn trait_generics(mut self, generics: impl ToTokens) -> Self {
        self.trait_generics = generics.to_token_stream();
        self
    }

    pub fn type_generics(mut self, generics: impl ToTokens) -> Self {
        self.type_generics = generics.to_token_stream();
        self
    }

    pub fn where_clause(mut self, wc: Option<syn::WhereClause>) -> Self {
        self.where_clause = wc;
        self
    }

    pub fn method(mut self, method: Method) -> Self {
        self.items.push(ImplItem::Method(method));
        self
    }

    pub fn assoc_type(mut self, name: syn::Ident, ty: impl ToTokens) -> Self {
        self.items.push(ImplItem::AssocType(AssocType {
            name,
            ty: ty.to_token_stream(),
        }));
        self
    }

    pub fn assoc_const(
        mut self,
        name: syn::Ident,
        ty: impl ToTokens,
        value: impl ToTokens,
    ) -> Self {
        self.items.push(ImplItem::AssocConst(AssocConst {
            name,
            ty: ty.to_token_stream(),
            value: value.to_token_stream(),
        }));
        self
    }
}

impl ToTokens for TraitImpl {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let (impl_generics, _, _) = self.generics.split_for_impl();
        let trait_path = &self.trait_path;
        let trait_generics = &self.trait_generics;
        let type_name = &self.type_name;
        let type_generics = &self.type_generics;
        let where_clause = &self.where_clause;
        let items = &self.items;

        tokens.extend(quote! {
            #[automatically_derived]
            impl #impl_generics #trait_path #trait_generics for #type_name #type_generics #where_clause {
                #(#items)*
            }
        });
    }
}

impl ToTokens for ImplItem {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        match self {
            ImplItem::Method(m) => m.to_tokens(tokens),
            ImplItem::AssocType(t) => t.to_tokens(tokens),
            ImplItem::AssocConst(c) => c.to_tokens(tokens),
        }
    }
}

impl ToTokens for Method {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let name = &self.name;
        let self_arg = &self.self_arg;
        let params = &self.params;
        let body = &self.body;
        let ret = match &self.return_type {
            Some(rt) => quote! { -> #rt },
            None => TokenStream::new(),
        };
        let comma = if params.is_empty() {
            TokenStream::new()
        } else {
            quote! { , }
        };
        tokens.extend(quote! {
            fn #name(#self_arg #comma #(#params),*) #ret {
                #body
            }
        });
    }
}

impl ToTokens for AssocType {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let name = &self.name;
        let ty = &self.ty;
        tokens.extend(quote! { type #name = #ty; });
    }
}

impl ToTokens for AssocConst {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let name = &self.name;
        let ty = &self.ty;
        let value = &self.value;
        tokens.extend(quote! { const #name: #ty = #value; });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::rustfmt_tokens;
    use quote::format_ident;

    #[test]
    fn trait_impl_with_method() {
        let ti = TraitImpl::new(
            syn::Generics::default(),
            quote! { MyTrait },
            quote! { MyType },
        )
        .method(Method {
            name: format_ident!("do_thing"),
            self_arg: quote! { &self },
            params: vec![],
            return_type: Some(quote! { bool }),
            body: quote! { true },
        });

        let output = rustfmt_tokens(&ti.to_token_stream());
        assert!(output.contains("impl MyTrait for MyType"));
        assert!(output.contains("fn do_thing(&self) -> bool"));
        assert!(output.contains("true"));
    }

    #[test]
    fn trait_impl_with_assoc_type_and_const() {
        let ti = TraitImpl::new(syn::Generics::default(), quote! { Foo }, quote! { Bar })
            .assoc_type(format_ident!("Output"), quote! { i32 })
            .assoc_const(format_ident!("COUNT"), quote! { usize }, quote! { 42 });

        let output = rustfmt_tokens(&ti.to_token_stream());
        assert!(output.contains("type Output = i32;"));
        assert!(output.contains("const COUNT: usize = 42;"));
    }

    #[test]
    fn method_with_params() {
        let m = Method {
            name: format_ident!("add"),
            self_arg: quote! { &self },
            params: vec![quote! { x: i32 }, quote! { y: i32 }],
            return_type: Some(quote! { i32 }),
            body: quote! { x + y },
        };

        let output = rustfmt_tokens(&m.to_token_stream());
        assert!(output.contains("fn add(&self, x: i32, y: i32) -> i32"));
    }

    #[test]
    fn method_without_return_type() {
        let m = Method {
            name: format_ident!("noop"),
            self_arg: quote! { &mut self },
            params: vec![],
            return_type: None,
            body: quote! {},
        };

        let output = m.to_token_stream().to_string();
        assert!(!output.contains("->"));
    }
}
