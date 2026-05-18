use kirin_derive_toolkit::ir::{Data, Input, StandardLayout};
use proc_macro2::TokenStream;
use quote::quote;

/// Generate `TryLiftFrom` and `TryProjectTo` impls for pure wrapper dialects.
///
/// - **Pure wrapper enum** (all variants have `#[wraps]`): generates one `TryLiftFrom<InnerTy>`
///   impl on the enum for each wrapped type, and one `TryProjectTo<InnerTy>` impl on the enum
///   for each variant.
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
                    let constructor = enum_variant_constructor(name, variant_name, wrapper);
                    let pattern = enum_variant_pattern(name, variant_name, wrapper);
                    let bridge = wrapper
                        .lift_project_from
                        .iter()
                        .map(|from_ty| {
                            let project_inner = quote! {
                                let inner = match self {
                                    #pattern => inner,
                                    _ => return Err(#crate_path::ProjectError::InvalidVariant),
                                };
                            };
                            generate_bridge_lift_project(
                                name,
                                &quote! { #constructor },
                                &project_inner,
                                inner_ty,
                                from_ty,
                                crate_path,
                                &impl_generics,
                                &ty_generics,
                                where_clause,
                            )
                        })
                        .collect::<TokenStream>();

                    quote! {
                        #[automatically_derived]
                        impl #impl_generics #crate_path::TryLiftFrom<#inner_ty> for #name #ty_generics
                        #where_clause
                        {
                            type Error = ::core::convert::Infallible;
                            fn try_lift_from(from: #inner_ty) -> ::core::result::Result<Self, Self::Error> {
                                Ok(#constructor)
                            }
                        }

                        #[automatically_derived]
                        impl #impl_generics #crate_path::TryProjectTo<#inner_ty> for #name #ty_generics
                        #where_clause
                        {
                            type Error = #crate_path::ProjectError;
                            fn try_project_to(self) -> ::core::result::Result<#inner_ty, Self::Error> {
                                match self {
                                    #pattern => Ok(inner),
                                    _ => Err(#crate_path::ProjectError::InvalidVariant),
                                }
                            }
                        }

                        #bridge
                    }
                })
                .collect()
        }
        Data::Struct(data) => {
            let Some(wrapper) = &data.wraps else {
                return TokenStream::new();
            };
            if !data.fields.is_empty() {
                return TokenStream::new();
            }
            let inner_ty = &wrapper.ty;

            let (lift_body, destruct) = if wrapper.field.ident.is_some() {
                let field_name = wrapper.field.name();
                (
                    quote! { #name { #field_name: from } },
                    quote! { #name { #field_name: inner } },
                )
            } else {
                (quote! { #name(from) }, quote! { #name(inner) })
            };
            let bridge = wrapper
                .lift_project_from
                .iter()
                .map(|from_ty| {
                    let project_inner = quote! {
                        let #destruct = self;
                    };
                    generate_bridge_lift_project(
                        name,
                        &lift_body,
                        &project_inner,
                        inner_ty,
                        from_ty,
                        crate_path,
                        &impl_generics,
                        &ty_generics,
                        where_clause,
                    )
                })
                .collect::<TokenStream>();

            quote! {
                #[automatically_derived]
                impl #impl_generics #crate_path::TryLiftFrom<#inner_ty> for #name #ty_generics
                #where_clause
                {
                    type Error = ::core::convert::Infallible;
                    fn try_lift_from(from: #inner_ty) -> ::core::result::Result<Self, Self::Error> {
                        Ok(#lift_body)
                    }
                }

                #[automatically_derived]
                impl #impl_generics #crate_path::TryProjectTo<#inner_ty> for #name #ty_generics
                #where_clause
                {
                    type Error = #crate_path::ProjectError;
                    fn try_project_to(self) -> ::core::result::Result<#inner_ty, Self::Error> {
                        let #destruct = self;
                        Ok(inner)
                    }
                }

                #bridge
            }
        }
    }
}

fn enum_variant_constructor(
    name: &syn::Ident,
    variant_name: &syn::Ident,
    wrapper: &kirin_derive_toolkit::ir::fields::Wrapper,
) -> TokenStream {
    if wrapper.field.ident.is_some() {
        let field_name = wrapper.field.name();
        quote! { #name::#variant_name { #field_name: from } }
    } else {
        quote! { #name::#variant_name(from) }
    }
}

fn enum_variant_pattern(
    name: &syn::Ident,
    variant_name: &syn::Ident,
    wrapper: &kirin_derive_toolkit::ir::fields::Wrapper,
) -> TokenStream {
    if wrapper.field.ident.is_some() {
        let field_name = wrapper.field.name();
        quote! { #name::#variant_name { #field_name: inner } }
    } else {
        quote! { #name::#variant_name(inner) }
    }
}

#[allow(clippy::too_many_arguments)]
fn generate_bridge_lift_project(
    name: &syn::Ident,
    lift_body: &TokenStream,
    project_inner: &TokenStream,
    inner_ty: &syn::Type,
    from_ty: &syn::Path,
    crate_path: &syn::Path,
    impl_generics: &syn::ImplGenerics<'_>,
    ty_generics: &syn::TypeGenerics<'_>,
    where_clause: Option<&syn::WhereClause>,
) -> TokenStream {
    quote! {
        #[automatically_derived]
        impl #impl_generics #crate_path::TryLiftFrom<#from_ty> for #name #ty_generics
        #where_clause
        {
            type Error = <#inner_ty as #crate_path::TryLiftFrom<#from_ty>>::Error;

            fn try_lift_from(from: #from_ty) -> ::core::result::Result<Self, Self::Error> {
                let from = <#inner_ty as #crate_path::TryLiftFrom<#from_ty>>::try_lift_from(from)?;
                Ok(#lift_body)
            }
        }

        #[automatically_derived]
        impl #impl_generics #crate_path::TryProjectTo<#from_ty> for #name #ty_generics
        #where_clause
        {
            type Error = #crate_path::ProjectError;

            fn try_project_to(self) -> ::core::result::Result<#from_ty, Self::Error> {
                #project_inner
                <#inner_ty as #crate_path::TryProjectTo<#from_ty>>::try_project_to(inner)
                    .map_err(|_| #crate_path::ProjectError::InvalidVariant)
            }
        }
    }
}
