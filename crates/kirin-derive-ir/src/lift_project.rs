use kirin_derive_toolkit::ir::{Data, Input, StandardLayout};
use proc_macro2::TokenStream;
use quote::quote;

/// Generate `Lift` and `Project` impls for pure wrapper dialects.
///
/// - **Pure wrapper enum** (all variants have `#[wraps]`): generates one `Lift<Enum>` impl
///   for each wrapped type, and one `Project<WrappedType>` impl on the enum for each variant.
/// - **Wrapper struct** (struct with `#[wraps]`): same, for the single inner type.
/// - **Non-pure-wrapper enum or non-wrapper struct**: emits nothing — Lift/Project algebra
///   is only derivable when the dialect is a transparent composition of sub-dialects.
pub(crate) fn generate_lift_project(
    ir: &Input<StandardLayout>,
    crate_path: &syn::Path,
) -> TokenStream {
    let name = &ir.name;
    let (impl_generics, ty_generics, where_clause) = ir.generics.split_for_impl();

    match &ir.data {
        Data::Enum(data) => {
            // A pure wrapper variant delegates entirely to its inner type with no side fields.
            let all_pure_wrappers = data
                .variants
                .iter()
                .all(|v| v.wraps.is_some() && v.fields.is_empty());
            if !all_pure_wrappers {
                return TokenStream::new();
            }

            data.variants
                .iter()
                .map(|variant| {
                    let wrapper = variant.wraps.as_ref().unwrap();
                    let variant_name = &variant.name;
                    let inner_ty = &wrapper.ty;

                    quote! {
                        #[automatically_derived]
                        impl #impl_generics #crate_path::Lift<#name #ty_generics> for #inner_ty
                        #where_clause
                        {
                            fn lift(self) -> #name #ty_generics {
                                #name::#variant_name(self)
                            }
                        }

                        #[automatically_derived]
                        impl #impl_generics #crate_path::Project<#inner_ty> for #name #ty_generics
                        #where_clause
                        {
                            fn try_project(self) -> ::core::result::Result<#inner_ty, Self> {
                                match self {
                                    #name::#variant_name(inner) => Ok(inner),
                                    other => Err(other),
                                }
                            }
                        }
                    }
                })
                .collect()
        }
        Data::Struct(data) => {
            let Some(wrapper) = &data.wraps else {
                return TokenStream::new();
            };
            let inner_ty = &wrapper.ty;

            let (lift_body, destruct) = if wrapper.field.ident.is_some() {
                let field_name = wrapper.field.name();
                (
                    quote! { #name { #field_name: self } },
                    quote! { #name { #field_name: inner } },
                )
            } else {
                (quote! { #name(self) }, quote! { #name(inner) })
            };

            quote! {
                #[automatically_derived]
                impl #impl_generics #crate_path::Lift<#name #ty_generics> for #inner_ty
                #where_clause
                {
                    fn lift(self) -> #name #ty_generics {
                        #lift_body
                    }
                }

                #[automatically_derived]
                impl #impl_generics #crate_path::Project<#inner_ty> for #name #ty_generics
                #where_clause
                {
                    fn try_project(self) -> ::core::result::Result<#inner_ty, Self> {
                        let #destruct = self;
                        Ok(inner)
                    }
                }
            }
        }
    }
}
