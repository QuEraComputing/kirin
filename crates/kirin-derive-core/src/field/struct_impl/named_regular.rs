use crate::field::data::{AccessorInfo, NamedMatchedFields};
use proc_macro2::TokenStream;

pub struct NamedStructRegularAccessor<'input> {
    info: &'input AccessorInfo,
    name: &'input syn::Ident,
    generics: &'input syn::Generics,
    matching_fields: NamedMatchedFields,
}

impl<'input> NamedStructRegularAccessor<'input> {
    pub fn scan(info: &'input AccessorInfo, input: &'input syn::DeriveInput, fields: &'input syn::FieldsNamed) -> Self {
        let matching_fields = NamedMatchedFields::new(&info, fields);
        Self {
            info,
            name: &input.ident,
            generics: &input.generics,
            matching_fields,
        }
    }

    pub fn generate(&self) -> TokenStream {
        let name = &self.name;
        let trait_path = &self.info.trait_path;
        let method_name = &self.info.name;
        let matching_fields = self.matching_fields.vars();
        let iter = self.matching_fields.iter();
        let iter_type = self.matching_fields.iter_type();
        let generics = self.info.generics(&self.generics);
        let (input_type_generics, lifetime, impl_generics, _, where_clause) =
            generics.split_for_impl();

        quote::quote! {
            impl #impl_generics #trait_path<#lifetime> for #name #input_type_generics #where_clause {
                type Iter = #iter_type;
                fn #method_name(&#lifetime self) -> Self::Iter {
                    let Self { #(#matching_fields,)* .. } = self;
                    #iter
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
    fn test_matching_one() {
        let input: syn::DeriveInput = syn::parse_quote! {
            struct TestStruct<T> {
                a: SSAValue,
                b: f64,
                c: T,
            }
        };
        let fields = extract_named_field(&input);
        let info = AccessorInfo::new("arguments", "SSAValue", "::kirin_ir::HasArguments");
        let accessor = NamedStructRegularAccessor::scan(&info, &input, fields);
        let generated = accessor.generate();
        insta::assert_snapshot!(rustfmt(generated))
    }

    #[test]
    fn test_matching_two() {
        let input: syn::DeriveInput = syn::parse_quote! {
            struct TestStruct<T> {
                a: SSAValue,
                b: SSAValue,
                c: T,
            }
        };
        let fields = extract_named_field(&input);
        let info = AccessorInfo::new("arguments", "SSAValue", "::kirin_ir::HasArguments");
        let accessor = NamedStructRegularAccessor::scan(&info, &input, fields);
        let generated = accessor.generate();
        insta::assert_snapshot!(rustfmt(generated))
    }

    #[test]
    fn test_matching_vec() {
        let input: syn::DeriveInput = syn::parse_quote! {
            struct TestStruct<T> {
                a: SSAValue,
                b: SSAValue,
                c: Vec<SSAValue>,
                d: T,
            }
        };
        let fields = extract_named_field(&input);
        let info = AccessorInfo::new("arguments", "SSAValue", "::kirin_ir::HasArguments");
        let accessor = NamedStructRegularAccessor::scan(&info, &input, fields);
        let generated = accessor.generate();
        insta::assert_snapshot!(rustfmt(generated))
    }

    pub fn extract_named_field(input: &syn::DeriveInput) -> &syn::FieldsNamed {
        if let syn::Data::Struct(syn::DataStruct {
            fields: syn::Fields::Named(fields),
            ..
        }) = &input.data
        {
            fields
        } else {
            panic!("Expected named fields");
        }
    }
}
