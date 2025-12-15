use proc_macro2::TokenStream;
use quote::{ToTokens, quote};

pub struct Receiver<'a> {
    pub mutable: bool,
    pub lifetime: Option<&'a syn::Lifetime>,
}

impl<'a> ToTokens for Receiver<'a> {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let ampersand = syn::token::And::default();
        let mutability = if self.mutable {
            Some(syn::token::Mut::default())
        } else {
            None
        };
        let lifetime = self.lifetime.as_ref();

        tokens.extend(quote! {
            #ampersand #lifetime #mutability self
        });
    }
}

/// Represents a <Ident>: <Type> expression
pub struct IdentType {
    pub ident: syn::Ident,
    pub ty: TokenStream,
}

impl ToTokens for IdentType {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let ident = &self.ident;
        let ty = &self.ty;

        tokens.extend(quote! {
            #ident: #ty
        });
    }
}

pub struct TraitItemFnImpl<'a> {
    pub ident: &'a syn::Ident,
    pub receiver: Receiver<'a>,
    pub generics: syn::Generics,
    pub arguments: Vec<IdentType>,
    pub body: Vec<TokenStream>,
    pub output: TokenStream,
}

impl<'a> TraitItemFnImpl<'a> {
    pub fn new(ident: &'a syn::Ident) -> Self {
        Self {
            ident: ident.into(),
            receiver: Receiver {
                mutable: false,
                lifetime: None,
            },
            generics: syn::Generics::default(),
            arguments: Vec::new(),
            body: Vec::new(),
            output: TokenStream::new(),
        }
    }

    pub fn with_mutable_self(mut self, mutable: bool) -> Self {
        self.receiver.mutable = mutable;
        self
    }

    pub fn with_self_lifetime(mut self, lifetime: &'a syn::Lifetime) -> Self {
        self.receiver.lifetime = Some(lifetime);
        self
    }

    pub fn with_generics(mut self, generics: syn::Generics) -> Self {
        self.generics = generics;
        self
    }

    pub fn with_argument(mut self, ident: syn::Ident, ty: impl ToTokens) -> Self {
        self.arguments.push(IdentType {
            ident,
            ty: ty.to_token_stream(),
        });
        self
    }

    pub fn with_output(mut self, output: impl ToTokens) -> Self {
        output.to_tokens(&mut self.output);
        self
    }

    pub fn with_token_body(mut self, body: impl quote::ToTokens) -> Self {
        self.body.push(body.into_token_stream());
        self
    }
}

impl<'a> ToTokens for TraitItemFnImpl<'a> {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let ident = &self.ident;
        let receiver = &self.receiver;
        let generics = &self.generics;
        let arguments = &self.arguments;
        let output = &self.output;
        let body = &self.body;

        tokens.extend(quote! {
            fn #ident #generics ( #receiver, #( #arguments ),* ) -> #output {
                #( #body )*
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use quote::format_ident;

    use super::*;

    #[test]
    fn test_trait_item_fn_impl_tokens() {
        let ident = format_ident!("my_method");
        let lifetime = syn::parse_quote!('a);
        let method = TraitItemFnImpl::new(&ident)
            .with_mutable_self(true)
            .with_self_lifetime(&lifetime)
            .with_generics(syn::parse_quote!(<'a, T>))
            .with_argument(syn::parse_str("arg1").unwrap(), quote! {i32})
            .with_argument(syn::parse_str("arg2").unwrap(), quote! {&str})
            .with_output(quote! {Result<(), Error>})
            .with_token_body(quote! {
                return something;
            });
        let generated_tokens = method.to_token_stream();
        let expected_tokens: proc_macro2::TokenStream = syn::parse_quote! {
            fn my_method<'a, T>(&'a mut self, arg1: i32, arg2: &str) -> Result<(), Error> {
                return something;
            }
        };
        assert_eq!(generated_tokens.to_string(), expected_tokens.to_string());
    }
}
