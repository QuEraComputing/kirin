use proc_macro2::TokenStream;

use crate::field::{
    data::AccessorInfo,
    enum_impl::variant::{NamedVariantRegularAccessor, UnnamedVariantRegularAccessor},
};

/// all variants are primitive instruction definitions
pub struct EnumRegularAccessor<'input> {
    name: &'input syn::Ident,
    generics: &'input syn::Generics,
    info: &'input AccessorInfo,
    variants: Vec<EnumRegularVariant<'input>>,
}

impl<'input> EnumRegularAccessor<'input> {
    pub fn scan(info: &'input AccessorInfo, input: &'input syn::DeriveInput, data: &'input syn::DataEnum) -> Self {
        let variants = data
            .variants
            .iter()
            .map(|variant| match &variant.fields {
                syn::Fields::Named(fields) => EnumRegularVariant::Named(
                    NamedVariantRegularAccessor::scan(info, input, &variant.ident, fields),
                ),
                syn::Fields::Unnamed(fields) => {
                    EnumRegularVariant::Unnamed(UnnamedVariantRegularAccessor::scan(
                        info,
                        input,
                        &variant.ident,
                        fields,
                    ))
                }
                _ => panic!("only named and unnamed fields are supported"),
            })
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

        let iter_variants = self.variants.iter().map(|variant| variant.iter_variant());
        let method_arms = self
            .variants
            .iter()
            .map(|variant| variant.method_arm())
            .collect::<Vec<_>>();
        let iter_next_arms = self.variants.iter().map(|variant| variant.iter_next_arm());

        let generics = self.info.generics(&self.generics);
        let (input_type_generics, lifetime, impl_generics, _, where_clause) =
            generics.split_for_impl();

        quote::quote! {
            impl #impl_generics #trait_path<#lifetime> for #name #input_type_generics #where_clause {
                type Iter = #iter_name<#lifetime, #matching_type>;
                pub fn #method_name(&#lifetime self) -> Self::Iter {
                    match self {
                        #(#method_arms)*
                    }
                }
            }

            // note that if only regular, we have no type parameters to forward
            pub enum #iter_name<#lifetime> {
                #(#iter_variants),*
            }

            impl<#lifetime> Iterator for #iter_name<#lifetime> {
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

pub enum EnumRegularVariant<'input> {
    Named(NamedVariantRegularAccessor<'input>),
    Unnamed(UnnamedVariantRegularAccessor<'input>),
}

impl<'input> EnumRegularVariant<'input> {
    pub fn iter_variant(&self) -> TokenStream {
        match self {
            EnumRegularVariant::Named(v) => v.iter_variant(),
            EnumRegularVariant::Unnamed(v) => v.iter_variant(),
        }
    }

    pub fn method_arm(&self) -> TokenStream {
        match self {
            EnumRegularVariant::Named(v) => v.method_arm(),
            EnumRegularVariant::Unnamed(v) => v.method_arm(),
        }
    }

    pub fn iter_next_arm(&self) -> TokenStream {
        match self {
            EnumRegularVariant::Named(v) => v.iter_next_arm(),
            EnumRegularVariant::Unnamed(v) => v.iter_next_arm(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::rustfmt;

    #[test]
    fn test_enum_regular_accessor() {
        let input: syn::DeriveInput = syn::parse_quote! {
            enum TestEnum<T> {
                VariantA { a: SSAValue, b: T, c: SSAValue },
                VariantB(SSAValue, f64, SSAValue),
            }
        };
        let data = match &input.data {
            syn::Data::Enum(data_enum) => data_enum,
            _ => panic!("expected enum"),
        };
        let info = AccessorInfo::new("arguments", "SSAValue", "::kirin_ir::HasArguments");
        let accessor = EnumRegularAccessor::scan(&info, &input, data);
        let generated = accessor.generate();
        insta::assert_snapshot!(rustfmt(generated))
    }
}
