//! Composition-enum dispatch and ordinary member injections.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{DeriveInput, parse_quote};

pub fn generate(input: &DeriveInput) -> syn::Result<TokenStream> {
    let path: syn::Path = parse_quote!(::kirin_interpreter);
    let syn::Data::Enum(data) = &input.data else {
        return Err(syn::Error::new_spanned(input, "Frame requires an enum"));
    };
    let mut members = Vec::new();
    for variant in &data.variants {
        let syn::Fields::Unnamed(fields) = &variant.fields else {
            return Err(syn::Error::new_spanned(
                variant,
                "expected one unnamed frame field",
            ));
        };
        if fields.unnamed.len() != 1 {
            return Err(syn::Error::new_spanned(
                variant,
                "expected one unnamed frame field",
            ));
        }
        members.push((&variant.ident, &fields.unnamed[0].ty));
    }
    let Some((_, first)) = members.first() else {
        return Err(syn::Error::new_spanned(
            input,
            "Frame requires at least one variant",
        ));
    };
    let name = &input.ident;
    let (original_impl, ty_generics, original_where) = input.generics.split_for_impl();
    let mut generics = input.generics.clone();
    generics.params.push(parse_quote!(__FrameEngine));
    let completion = quote!(<#first as #path::Frame<__FrameEngine, Self>>::Completion);
    let predicates = &mut generics.make_where_clause().predicates;
    predicates.push(parse_quote!(__FrameEngine: #path::FrameEngine));
    predicates.push(parse_quote!(#first: #path::Frame<__FrameEngine, Self>));
    for (_, ty) in members.iter().skip(1) {
        predicates.push(parse_quote!(
            #ty: #path::Frame<__FrameEngine, Self, Completion = #completion>
        ));
    }
    let (impl_generics, _, where_clause) = generics.split_for_impl();
    let conversions = members.iter().map(|(variant, ty)| {
        quote! {
            #[automatically_derived]
            impl #original_impl ::core::convert::From<#ty> for #name #ty_generics #original_where {
                fn from(frame: #ty) -> Self { Self::#variant(frame) }
            }
        }
    });
    let methods = ["step_into", "resume_done_into", "resume_into"].map(|method| {
        let method = syn::Ident::new(method, proc_macro2::Span::call_site());
        let (parameter, argument) = if method == "resume_into" {
            (quote!(completion: Self::Completion,), quote!(completion,))
        } else {
            (quote!(), quote!())
        };
        let arms = members.iter().map(|(variant, ty)| {
            quote! {
                Self::#variant(frame) =>
                    <#ty as #path::Frame<__FrameEngine, Self>>::#method(frame, #argument interp)
                        .map(|effect| effect.map_next(Self::#variant)),
            }
        });
        quote! {
            fn #method(self, #parameter interp: &mut __FrameEngine)
                -> ::core::result::Result<
                    #path::FrameEffect<Self, Self::Completion>,
                    <__FrameEngine as #path::FrameEngine>::Error,
                >
            {
                match self { #(#arms)* }
            }
        }
    });
    Ok(quote! {
        #(#conversions)*
        #[automatically_derived]
        impl #impl_generics #path::Frame<__FrameEngine> for #name #ty_generics #where_clause {
            type Completion = #completion;
            #(#methods)*
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use kirin_test_utils::rustfmt;

    #[test]
    fn member_conversions_and_dispatch() {
        let input = syn::parse_quote! {
            enum TestFrame {
                Block(BlockFrame),
                ScfFor(ScfForFrame),
            }
        };
        let tokens = generate(&input).expect("codegen failed");
        insta::assert_snapshot!(rustfmt(tokens.to_string()));
    }
}
