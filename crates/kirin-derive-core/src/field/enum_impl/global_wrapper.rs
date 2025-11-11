use proc_macro2::TokenStream;
use quote::quote;

use crate::field::{
    data::AccessorInfo,
    enum_impl::variant::{NamedVariantWrapperAccessor, UnnamedVariantWrapperAccessor},
};

/// all variants are wrapper of some other instruction
pub struct EnumGlobalWrapperAccessor<'input> {
    name: &'input syn::Ident,
    generics: &'input syn::Generics,
    info: &'input AccessorInfo,
    variants: Vec<EnumGlobalVariant<'input>>,
}

pub enum EnumGlobalVariant<'input> {
    Named(NamedVariantWrapperAccessor<'input>),
    Unnamed(UnnamedVariantWrapperAccessor<'input>),
}

impl<'input> EnumGlobalVariant<'input> {
    pub fn scan(info: &'input AccessorInfo, input: &'input syn::DeriveInput, variant: &'input syn::Variant) -> Self {
        if let syn::Fields::Named(fields_named) = &variant.fields {
            EnumGlobalVariant::Named(NamedVariantWrapperAccessor::scan(
                info,
                input,
                &variant.ident,
                fields_named,
            ))
        } else if let syn::Fields::Unnamed(fields_unnamed) = &variant.fields {
            EnumGlobalVariant::Unnamed(UnnamedVariantWrapperAccessor::scan(
                info,
                input,
                &variant.ident,
                fields_unnamed,
            ))
        } else {
            panic!("unit variants are not supported");
        }
    }
}

impl<'input> EnumGlobalWrapperAccessor<'input> {
    pub fn scan(
        info: &'input AccessorInfo,
        input: &'input syn::DeriveInput,
        data: &'input syn::DataEnum,
    ) -> Self {
        let variants = data
            .variants
            .iter()
            .map(|v| EnumGlobalVariant::scan(info, input, v))
            .collect();
        Self {
            name: &input.ident,
            generics: &input.generics,
            info,
            variants,
        }
    }

    pub fn generate(&self) -> TokenStream {
        let name = &self.name;
        let method_name = &self.info.name;
        let trait_path = &self.info.trait_path;
        let iter_name = &self.info.iter_name;
        let matching_type = &self.info.matching_type;

        let iter_variants = self.variants.iter().map(|variant| match variant {
            EnumGlobalVariant::Named(v) => v.iter_variant(),
            EnumGlobalVariant::Unnamed(v) => v.iter_variant(),
        });
        let method_arms = self.variants.iter().map(|variant| match variant {
            EnumGlobalVariant::Named(v) => v.method_arm(),
            EnumGlobalVariant::Unnamed(v) => v.method_arm(),
        });
        let iter_next_arms = self.variants.iter().map(|variant| match variant {
            EnumGlobalVariant::Named(v) => v.iter_next_arm(),
            EnumGlobalVariant::Unnamed(v) => v.iter_next_arm(),
        });

        let generics = self.info.generics(&self.generics);
        let iter_generics = generics.generics.clone();
        let (input_type_generics, lifetime, impl_generics, ty_generics, where_clause) =
            generics.split_for_impl();

        quote! {
            impl #impl_generics #trait_path<#lifetime> for #name #input_type_generics #where_clause {
                type Iter = #iter_name<#lifetime>;
                pub fn #method_name(&#lifetime self) -> Self::Iter {
                    match self {
                        #(#method_arms)*
                    }
                }
            }

            pub enum #iter_name #iter_generics {
                #(#iter_variants),*
            }

            impl #impl_generics Iterator for #iter_name #ty_generics #where_clause {
                type Item = &#lifetime #matching_type;
                fn next(&mut self) -> Option<Self::Item> {
                    match self {
                        #(#iter_next_arms)*
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::rustfmt;
    #[test]
    fn test_enum_global_wrapper_accessor() {
        let input: syn::DeriveInput = syn::parse_quote! {
            enum TestEnum<T> {
                VariantA { wrapped: InnerStructA<T> },
                VariantB(InnerStructB),
            }
        };
        let data = match &input.data {
            syn::Data::Enum(data_enum) => data_enum,
            _ => panic!("expected enum"),
        };
        let info = AccessorInfo::new("arguments", "SSAValue", "::kirin_ir::HasArguments");
        let accessor = EnumGlobalWrapperAccessor::scan(&info, &input, data);
        let generated = accessor.generate();
        insta::assert_snapshot!(rustfmt(generated))
    }
}
