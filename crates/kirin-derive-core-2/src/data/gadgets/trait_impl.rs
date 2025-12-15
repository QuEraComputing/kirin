/// gadgets for generating trait implementations
/// derive macros, or other quote-based code generation.
use proc_macro2::TokenStream;
use quote::{ToTokens, quote};
pub struct TraitTypeImpl {
    pub ident: TokenStream,
    pub ty: TokenStream,
}

impl ToTokens for TraitTypeImpl {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let ident = &self.ident;
        let ty = &self.ty;
        tokens.extend(quote! {
            type #ident = #ty;
        });
    }
}

pub struct TraitImpl<'a> {
    pub input: &'a syn::DeriveInput,
    pub trait_name: &'a syn::Ident,
    pub trait_generics: &'a syn::Generics,
    pub combined_generics: syn::Generics,
    pub types: Vec<TraitTypeImpl>,
    pub methods: TokenStream,
}

impl<'a> TraitImpl<'a> {
    /// create a new TraitImpl for the given input and trait
    /// name and generics
    pub fn new(
        input: &'a syn::DeriveInput,
        trait_name: &'a syn::Ident,
        trait_generics: &'a syn::Generics,
    ) -> Self {
        let combined_generics = {
            let mut combined = input.generics.clone();
            for param in trait_generics.params.iter() {
                combined.params.push(param.clone());
            }
            combined
        };
        Self {
            input,
            trait_name,
            trait_generics,
            combined_generics,
            types: Vec::new(),
            methods: TokenStream::new(),
        }
    }

    /// add a associated type to the trait implementation
    pub fn add_type(mut self, ident: impl ToTokens, ty: impl ToTokens) -> Self {
        self.types.push(TraitTypeImpl {
            ident: ident.to_token_stream(),
            ty: ty.to_token_stream(),
        });
        self
    }

    /// add a method to the trait implementation
    pub fn add_method(mut self, method: impl ToTokens) -> Self {
        method.to_tokens(&mut self.methods);
        self
    }
}

impl ToTokens for TraitImpl<'_> {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let input_name = &self.input.ident;
        let trait_name = self.trait_name;
        let (combined_impl_generics, _combined_ty_generics, combined_where_clause) =
            self.combined_generics.split_for_impl();
        let (_input_impl_generics, input_ty_generics, _input_where_clause) =
            self.input.generics.split_for_impl();
        let (_trait_impl_generics, trait_ty_generics, _trait_where_clause) =
            self.trait_generics.split_for_impl();
        let types = &self.types;
        let methods = &self.methods;

        tokens.extend(quote! {
            #[automatically_derived]
            impl #combined_impl_generics #trait_name #trait_ty_generics for #input_name #input_ty_generics
            #combined_where_clause {
                #(#types)*
                #methods
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use quote::format_ident;

    use crate::data::gadgets::TraitItemFnImpl;

    use super::*;

    #[test]
    fn test_trait_impl_generation() {
        let input: syn::DeriveInput = syn::parse_quote! {
            struct MyStruct<T> {
                a: T,
            }
        };

        let trait_name: syn::Ident = syn::parse_str("MyTrait").unwrap();
        let trait_lifetime = syn::parse_str("'a").unwrap();
        let trait_generics: syn::Generics = syn::parse_str("<'a, U>").unwrap();
        let trait_method = format_ident!("trait_method");
        let trait_impl = TraitImpl::new(&input, &trait_name, &trait_generics)
            .add_type(format_ident!("Iter"), quote! { i64 })
            .add_method(
                TraitItemFnImpl::new(&trait_method)
                    .with_mutable_self(true)
                    .with_self_lifetime(&trait_lifetime)
                    .with_argument(format_ident!("x"), quote!(i64))
                    .with_argument(format_ident!("y"), quote!(f64))
                    .with_token_body(quote! {
                        return something;
                    }),
            );
        let generated_tokens = trait_impl.to_token_stream();
        let expected_tokens: proc_macro2::TokenStream = syn::parse_quote! {
            #[automatically_derived]
            impl<'a, T, U> MyTrait<'a, U> for MyStruct<T> {
                type Iter = i64;
                fn trait_method(&'a mut self, x: i64, y: f64) {
                    return something;
                }
            }
        };
        assert_eq!(generated_tokens.to_string(), expected_tokens.to_string());
    }
}
