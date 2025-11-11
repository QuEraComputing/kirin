use proc_macro2::TokenStream;

use crate::field::{
    data::{AccessorInfo, has_attr},
    enum_impl::variant::{
        NamedVariantRegularAccessor, NamedVariantWrapperAccessor, UnnamedVariantRegularAccessor,
        UnnamedVariantWrapperAccessor,
    },
};

/// some variants are wrapper of other instructions
/// some are primitive instruction definitions
pub struct EnumSomeWrapperAccessor<'input> {
    info: &'input AccessorInfo,
    name: &'input syn::Ident,
    generics: &'input syn::Generics,
    variants: Vec<EnumSomeWrapperVariant<'input>>,
}

impl<'input> EnumSomeWrapperAccessor<'input> {
    pub fn scan(info: &'input AccessorInfo, input: &'input syn::DeriveInput, data: &'input syn::DataEnum) -> Self {
        let variants = data
            .variants
            .iter()
            .map(|variant| EnumSomeWrapperVariant::scan(info, input, variant))
            .collect();

        Self {
            info,
            name: &input.ident,
            generics: &input.generics,
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
        let iter_generics = generics.generics.clone();
        let (input_type_generics, lifetime, impl_generics, ty_generics, where_clause) =
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

pub enum EnumSomeWrapperVariant<'input> {
    NamedRegular(NamedVariantRegularAccessor<'input>),
    NamedWrapper(NamedVariantWrapperAccessor<'input>),
    UnnamedRegular(UnnamedVariantRegularAccessor<'input>),
    UnnamedWrapper(UnnamedVariantWrapperAccessor<'input>),
}

impl<'input> EnumSomeWrapperVariant<'input> {
    pub fn scan(info: &'input AccessorInfo, input: &'input syn::DeriveInput, variant: &'input syn::Variant) -> Self {
        match &variant.fields {
            syn::Fields::Named(fields_named) => {
                if fields_named.named.len() == 1
                    || fields_named
                        .named
                        .iter()
                        .any(|f| has_attr(&f.attrs, "kirin", "wraps"))
                {
                    EnumSomeWrapperVariant::NamedWrapper(NamedVariantWrapperAccessor::scan(
                        info,
                        input,
                        &variant.ident,
                        fields_named,
                    ))
                } else {
                    EnumSomeWrapperVariant::NamedRegular(NamedVariantRegularAccessor::scan(
                        info,
                        input,
                        &variant.ident,
                        fields_named,
                    ))
                }
            }
            syn::Fields::Unnamed(fields_unnamed) => {
                if fields_unnamed.unnamed.len() == 1
                    || fields_unnamed
                        .unnamed
                        .iter()
                        .any(|f| has_attr(&f.attrs, "kirin", "wraps"))
                {
                    EnumSomeWrapperVariant::UnnamedWrapper(UnnamedVariantWrapperAccessor::scan(
                        info,
                        input,
                        &variant.ident,
                        fields_unnamed,
                    ))
                } else {
                    EnumSomeWrapperVariant::UnnamedRegular(UnnamedVariantRegularAccessor::scan(
                        info,
                        input,
                        &variant.ident,
                        fields_unnamed,
                    ))
                }
            }
            _ => panic!("only named and unnamed fields are supported"),
        }
    }

    pub fn iter_variant(&self) -> TokenStream {
        match self {
            EnumSomeWrapperVariant::NamedRegular(v) => v.iter_variant(),
            EnumSomeWrapperVariant::NamedWrapper(v) => v.iter_variant(),
            EnumSomeWrapperVariant::UnnamedRegular(v) => v.iter_variant(),
            EnumSomeWrapperVariant::UnnamedWrapper(v) => v.iter_variant(),
        }
    }

    pub fn method_arm(&self) -> TokenStream {
        match self {
            EnumSomeWrapperVariant::NamedRegular(v) => v.method_arm(),
            EnumSomeWrapperVariant::NamedWrapper(v) => v.method_arm(),
            EnumSomeWrapperVariant::UnnamedRegular(v) => v.method_arm(),
            EnumSomeWrapperVariant::UnnamedWrapper(v) => v.method_arm(),
        }
    }

    pub fn iter_next_arm(&self) -> TokenStream {
        match self {
            EnumSomeWrapperVariant::NamedRegular(v) => v.iter_next_arm(),
            EnumSomeWrapperVariant::NamedWrapper(v) => v.iter_next_arm(),
            EnumSomeWrapperVariant::UnnamedRegular(v) => v.iter_next_arm(),
            EnumSomeWrapperVariant::UnnamedWrapper(v) => v.iter_next_arm(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::rustfmt;

    #[test]
    fn test_enum_some_wrapper_accessor() {
        let input: syn::DeriveInput = syn::parse_quote! {
            enum TestEnum<T> {
                VariantA { wrapped: InnerStructA<T> },
                VariantB(InnerStructB),
                VariantC { a: SSAValue, b: T, c: SSAValue },
                VariantD(SSAValue, f64, SSAValue),
            }
        };
        let data = match &input.data {
            syn::Data::Enum(data_enum) => data_enum,
            _ => panic!("expected enum"),
        };
        let info = AccessorInfo::new("arguments", "SSAValue", "::kirin_ir::HasArguments");
        let accessor = EnumSomeWrapperAccessor::scan(&info, &input, data);
        let generated = accessor.generate();
        insta::assert_snapshot!(rustfmt(generated))
    }
}

