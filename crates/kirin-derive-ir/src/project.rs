use kirin_derive_toolkit::{
    codegen::SingleField,
    ir::{Data, Input, StandardLayout},
};
use proc_macro2::TokenStream;
use quote::quote;

/// Generate `TryFrom` and `TryProjectTo` impls for pure wrapper dialects.
///
/// - **Pure wrapper enum** (all variants have `#[wraps]`): generates one `TryFrom<InnerTy>`
///   impl on the enum for each wrapped type, and one `TryProjectTo<InnerTy>` impl on the enum
///   for each variant.
/// - **Wrapper struct** (struct with `#[wraps]`): same, for the single inner type.
/// - **Non-pure-wrapper enum or non-wrapper struct**: emits nothing — Into/Project algebra
///   is only derivable when the dialect is a transparent composition of sub-dialects.
pub(crate) fn generate_project(ir: &Input<StandardLayout>, crate_path: &syn::Path) -> TokenStream {
    let name = &ir.name;
    let builder = LiftProjectImplBuilder::new(name, &ir.generics, crate_path);
    // When `#[kirin(builders)]` is enabled, the builder template emits
    // `impl From<Inner> for Self` for each wrapper variant. Skip emitting
    // it here to avoid coherence conflicts; otherwise emit it ourselves.
    let emit_direct_from = ir.attrs.builder.is_none();

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
                    let direct = builder.direct(&constructor, &pattern, inner_ty, emit_direct_from);
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
                            builder.bridge(
                                &quote! { #constructor },
                                &project_inner,
                                inner_ty,
                                from_ty,
                            )
                        })
                        .collect::<TokenStream>();

                    quote! {
                        #direct
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
            let (impl_generics, ty_generics, where_clause) = ir.generics.split_for_impl();

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
                    builder.bridge(&lift_body, &project_inner, inner_ty, from_ty)
                })
                .collect::<TokenStream>();

            let from_impl = if emit_direct_from {
                quote! {
                    #[automatically_derived]
                    impl #impl_generics ::core::convert::From<#inner_ty> for #name #ty_generics
                    #where_clause
                    {
                        fn from(from: #inner_ty) -> Self {
                            #lift_body
                        }
                    }
                }
            } else {
                quote! {}
            };

            quote! {
                #from_impl

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

pub(crate) fn generate_wrapper_enum_direct(
    input: &syn::DeriveInput,
    crate_path: &syn::Path,
) -> syn::Result<TokenStream> {
    let name = &input.ident;
    let builder = LiftProjectImplBuilder::new(name, &input.generics, crate_path);
    let syn::Data::Enum(data) = &input.data else {
        return Err(syn::Error::new_spanned(
            input,
            "derivation only supports enum inputs",
        ));
    };

    let impls = data
        .variants
        .iter()
        .map(|variant| {
            let field = SingleField::from_fields(&variant.fields)?;
            let variant_name = &variant.ident;
            let inner_ty = &field.ty;
            let binding = syn::Ident::new("from", variant_name.span());
            let inner = syn::Ident::new("inner", variant_name.span());
            let constructor = field.constructor(&binding);
            let pattern = field.pattern(&inner);
            let lift_body = quote! { Self::#variant_name #constructor };
            let project_pattern = quote! { Self::#variant_name #pattern };
            Ok(builder.direct(&lift_body, &project_pattern, inner_ty, true))
        })
        .collect::<syn::Result<Vec<_>>>()?;

    Ok(quote! {
        #(#impls)*
    })
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

struct LiftProjectImplBuilder<'a> {
    name: &'a syn::Ident,
    crate_path: &'a syn::Path,
    impl_generics: syn::ImplGenerics<'a>,
    ty_generics: syn::TypeGenerics<'a>,
    where_clause: Option<&'a syn::WhereClause>,
}

impl<'a> LiftProjectImplBuilder<'a> {
    fn new(name: &'a syn::Ident, generics: &'a syn::Generics, crate_path: &'a syn::Path) -> Self {
        let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
        Self {
            name,
            crate_path,
            impl_generics,
            ty_generics,
            where_clause,
        }
    }

    fn direct(
        &self,
        lift_body: &TokenStream,
        project_pattern: &TokenStream,
        inner_ty: &syn::Type,
        emit_from: bool,
    ) -> TokenStream {
        let name = self.name;
        let crate_path = self.crate_path;
        let impl_generics = &self.impl_generics;
        let ty_generics = &self.ty_generics;
        let where_clause = self.where_clause;

        let from_impl = if emit_from {
            quote! {
                #[automatically_derived]
                impl #impl_generics ::core::convert::From<#inner_ty> for #name #ty_generics
                #where_clause
                {
                    fn from(from: #inner_ty) -> Self {
                        #lift_body
                    }
                }
            }
        } else {
            quote! {}
        };

        quote! {
            #from_impl

            #[automatically_derived]
            impl #impl_generics #crate_path::TryProjectTo<#inner_ty> for #name #ty_generics
            #where_clause
            {
                type Error = #crate_path::ProjectError;
                fn try_project_to(self) -> ::core::result::Result<#inner_ty, Self::Error> {
                    match self {
                        #project_pattern => Ok(inner),
                        _ => Err(#crate_path::ProjectError::InvalidVariant),
                    }
                }
            }
        }
    }

    fn bridge(
        &self,
        lift_body: &TokenStream,
        project_inner: &TokenStream,
        inner_ty: &syn::Type,
        from_ty: &syn::Path,
    ) -> TokenStream {
        let name = self.name;
        let crate_path = self.crate_path;
        let impl_generics = &self.impl_generics;
        let ty_generics = &self.ty_generics;
        let where_clause = self.where_clause;

        quote! {
            #[automatically_derived]
            impl #impl_generics ::core::convert::From<#from_ty> for #name #ty_generics
            #where_clause
            {
                fn from(from: #from_ty) -> Self {
                    let from = <#inner_ty as ::core::convert::From<#from_ty>>::from(from);
                    #lift_body
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
}
