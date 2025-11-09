use proc_macro2::TokenStream;
use quote::quote;

use crate::{DeriveContext, DeriveHelperAttribute, accessor::config::Config};

pub struct AccessorImpl {
    name: syn::Ident,
    accessor_name: syn::Ident,
    matching_type: syn::Ident,
    iter_name: syn::Ident,
    trait_path: syn::Path,
    inner: AccessorImplInner,
}

enum AccessorImplInner {
    NoWraps,
    Struct,
    Enum(bool, Vec<syn::Ident>),
}

impl AccessorImpl {
    pub fn new<A: DeriveHelperAttribute>(config: &Config, ctx: &DeriveContext<A>) -> Self {
        let name = ctx.input.ident.clone();
        let accessor_name = config.accessor.clone();
        let matching_type = config.matching_type.clone();
        let iter_name = config.accessor_iter.clone();

        let inner = match &ctx.input.data {
            syn::Data::Struct(_) => {
                if ctx.global_wraps() {
                    AccessorImplInner::Struct
                } else {
                    AccessorImplInner::NoWraps
                }
            }
            syn::Data::Enum(data) => {
                let global_wraps = ctx.global_wraps();
                let wrapper_variants = data
                    .variants
                    .iter()
                    .filter_map(|variant| {
                        if global_wraps || ctx.variant_wraps(&variant.ident) {
                            Some(variant.ident.clone())
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>();

                if wrapper_variants.is_empty() && !global_wraps {
                    AccessorImplInner::NoWraps
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
            NoWraps => {
                quote! {
                    fn #accessor_name(&self) -> impl Iterator<Item = ::kirin_ir::#matching_type> {
                        #iter_name {
                            parent: self,
                            index: 0,
                        }
                    }
                }
            }
            Struct => {
                quote! {
                    fn #accessor_name(&self) -> impl Iterator<Item = ::kirin_ir::#matching_type> {
                        let #name (wrapped_instruction) = self;
                        <wrapped_instruction as #trait_path>::#accessor_name()
                    }
                }
            }
            Enum(global_wraps, wrapper_variants) => {
                let arms = wrapper_variants.iter().map(|variant| {
                    quote! {
                        Self::#variant (wrapped_instruction) => {
                            <wrapped_instruction as #trait_path>::#accessor_name()
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
                    fn #accessor_name(&self) -> impl Iterator<Item = ::kirin_ir::#matching_type> {
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
