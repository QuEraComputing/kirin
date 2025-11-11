use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};

use crate::{
    DeriveContext, DeriveHelperAttribute,
    accessor::{config::Config, iterator::has_container_and_only_one_container},
};

pub struct AccessorImpl {
    name: syn::Ident,
    accessor_name: syn::Ident,
    matching_type: syn::Ident,
    iter_name: syn::Ident,
    trait_path: syn::Path,
    inner: AccessorImplInner,
}

impl AccessorImpl {
    pub fn new<A: DeriveHelperAttribute>(config: &Config, ctx: &DeriveContext<A>) -> Self {
        let name = ctx.input.ident.clone();
        let accessor_name = config.accessor.clone();
        let matching_type = config.matching_type.clone();
        let iter_name = config.accessor_iter.clone();

        let inner = match &ctx.input.data {
            syn::Data::Struct(data) => {
                AccessorImplInner::Struct(StructInfo::new(ctx.global_wraps(), data, &matching_type))
            }
            syn::Data::Enum(data) => {
                let global_wraps = ctx.global_wraps();
                let wrapper_variants = data
                    .variants
                    .iter()
                    .map(|variant| {
                        VariantInfo::new(
                            global_wraps || ctx.variant_wraps(&variant.ident),
                            variant,
                            &matching_type,
                        )
                    })
                    .collect::<Vec<_>>();

                if wrapper_variants.is_empty() && !global_wraps {
                    AccessorImplInner::NoWrapsNoContainer
                } else {
                    AccessorImplInner::Enum(global_wraps, wrapper_variants)
                }
            }
            _ => panic!("only structs and enums are supported"),
        };

        Self {
            name,
            accessor_name,
            matching_type,
            iter_name,
            trait_path: config.trait_path.clone(),
            inner,
        }
    }

    pub fn generate(&self) -> TokenStream {
        let name = &self.name;
        let accessor_name = &self.accessor_name;
        let matching_type = &self.matching_type;
        let iter_name = &self.iter_name;
        let trait_path = &self.trait_path;

        use AccessorImplInner::*;
        match &self.inner {
            NoWrapsNoContainer | Struct(StructInfo::NoWraps) => {
                quote! {
                    fn #accessor_name(&self) -> impl Iterator<Item = &::kirin_ir::#matching_type> {
                        #iter_name {
                            parent: self,
                            index: 0,
                        }
                    }
                }
            }
            Enum(false, info) if info.iter().all(|i| matches!(i, VariantInfo::NoWraps)) => {
                quote! {
                    fn #accessor_name(&self) -> impl Iterator<Item = &::kirin_ir::#matching_type> {
                        #iter_name {
                            parent: self,
                            index: 0,
                        }
                    }
                }
            }
            Struct(StructInfo::Wrapper) => {
                quote! {
                    fn #accessor_name(&self) -> impl Iterator<Item = &::kirin_ir::#matching_type> {
                        let #name (wrapped_instruction) = self;
                        <wrapped_instruction as #trait_path>::#accessor_name(wrapped_instruction)
                    }
                }
            }
            Struct(StructInfo::Anonymous(total, container)) => {
                let vars = (0..*total)
                    .map(|i| format_ident!("field_{}", i))
                    .collect::<Vec<_>>();
                let container_field = format_ident!("field_{}", container);
                quote! {
                    fn #accessor_name(&self) -> impl Iterator<Item = &::kirin_ir::#matching_type> {
                        let #name (#(#vars),*) = self;
                        #container_field.iter()
                    }
                }
            }
            Struct(StructInfo::Named(container)) => {
                quote! {
                    fn #accessor_name(&self) -> impl Iterator<Item = &::kirin_ir::#matching_type> {
                        let #name { #container, .. } = self;
                        #container.iter()
                    }
                }
            }
            Enum(global_wraps, wrapper_variants) => {
                let arms = wrapper_variants.iter().map(|variant| match variant {
                    VariantInfo::NoWraps => quote! {},
                    VariantInfo::Wrapper(variant_name) => {
                        quote! {
                            Self::#variant_name (wrapped_instruction) => {
                                <wrapped_instruction as #trait_path>::#accessor_name()
                            }
                        }
                    }
                    VariantInfo::Anonymous {
                        name,
                        total,
                        container,
                    } => {
                        let vars = (0..*total)
                            .map(|i| format_ident!("field_{}", i))
                            .collect::<Vec<_>>();
                        quote! {
                            Self::#name ( #(#vars),* ) => {
                                #container.iter()
                            }
                        }
                    }
                    VariantInfo::Named { name, container } => {
                        quote! {
                            Self::#name { #container, .. } => {
                                #container.iter()
                            }
                        }
                    }
                });
                let other_arm = if *global_wraps {
                    quote! {}
                } else {
                    quote! {
                        _ => {
                            #iter_name {
                                parent: self,
                                index: 0,
                            }
                        }
                    }
                };

                quote! {
                    fn #accessor_name(&self) -> impl Iterator<Item = &::kirin_ir::#matching_type> {
                        match self {
                            #(#arms)*
                            #other_arm
                        }
                    }
                }
            }
        }
    }
}

enum AccessorImplInner {
    /// No wrapping and no container, just generate iterator directly
    NoWrapsNoContainer,
    Struct(StructInfo),
    /// (global_wraps, variants)
    Enum(bool, Vec<VariantInfo>),
}

enum StructInfo {
    NoWraps,
    Wrapper,
    /// named field being a container
    Named(syn::Ident),
    /// unnamed field being a container
    Anonymous(usize, usize),
}

impl StructInfo {
    pub fn new(global_wraps: bool, data: &syn::DataStruct, typename: &syn::Ident) -> Self {
        if global_wraps {
            return Self::Wrapper;
        }
        match &data.fields {
            syn::Fields::Named(fields_named) => {
                if let Some((_, container_name)) = has_container_and_only_one_container(
                    &fields_named.named,
                    typename.to_string().as_str(),
                ) {
                    Self::Named(container_name.unwrap())
                } else {
                    Self::NoWraps
                }
            }
            syn::Fields::Unnamed(fields_unnamed) => {
                if let Some((container_index, _)) = has_container_and_only_one_container(
                    &fields_unnamed.unnamed,
                    typename.to_string().as_str(),
                ) {
                    Self::Anonymous(fields_unnamed.unnamed.len(), container_index)
                } else {
                    Self::NoWraps
                }
            }
            syn::Fields::Unit => Self::NoWraps,
        }
    }
}

enum VariantInfo {
    NoWraps,
    Named {
        name: syn::Ident,
        container: syn::Ident,
    },
    Anonymous {
        name: syn::Ident,
        total: usize,
        container: syn::LitInt,
    },
    Wrapper(syn::Ident),
}

impl VariantInfo {
    pub fn new(wraps: bool, variant: &syn::Variant, typename: &syn::Ident) -> Self {
        if wraps {
            return Self::Wrapper(variant.ident.clone());
        }
        match &variant.fields {
            syn::Fields::Named(fields_named) => {
                if let Some((_, container_name)) = has_container_and_only_one_container(
                    &fields_named.named,
                    typename.to_string().as_str(),
                ) {
                    Self::Named {
                        name: variant.ident.clone(),
                        container: container_name.unwrap(),
                    }
                } else {
                    Self::NoWraps
                }
            }
            syn::Fields::Unnamed(fields_unnamed) => {
                if let Some((container_index, _)) = has_container_and_only_one_container(
                    &fields_unnamed.unnamed,
                    typename.to_string().as_str(),
                ) {
                    Self::Anonymous {
                        name: variant.ident.clone(),
                        total: fields_unnamed.unnamed.len(),
                        container: syn::LitInt::new(
                            &container_index.to_string(),
                            Span::call_site(),
                        ),
                    }
                } else {
                    Self::NoWraps
                }
            }
            syn::Fields::Unit => Self::NoWraps,
        }
    }
}
