use proc_macro2::TokenStream;

use crate::field::data::{AccessorInfo, UnnamedMatchedFields};

pub struct UnnamedStructRegularAccessor<'input> {
    pub info: &'input AccessorInfo,
    pub name: &'input syn::Ident,
    pub generics: &'input syn::Generics,
    pub total_fields: usize,
    pub matching_fields: UnnamedMatchedFields,
}

impl<'input> UnnamedStructRegularAccessor<'input> {
    pub fn scan(info: &'input AccessorInfo, input: &'input syn::DeriveInput, fields: &syn::FieldsUnnamed) -> Self {
        let matching_fields = UnnamedMatchedFields::new(&info, fields);
        Self {
            info,
            name: &input.ident,
            generics: &input.generics,
            total_fields: fields.unnamed.len(),
            matching_fields,
        }
    }

    pub fn generate(&self) -> TokenStream {
        let name = &self.name;
        let trait_path = &self.info.trait_path;
        let method_name = &self.info.name;
        let vars = self.matching_fields.vars();
        let iter = self.matching_fields.iter();
        let iter_type = self.matching_fields.iter_type();
        let generics = self.info.generics(&self.generics);
        let (input_type_generics, lifetime, impl_generics, _, where_clause) =
            generics.split_for_impl();

        quote::quote! {
            impl #impl_generics #trait_path<#lifetime> for #name #input_type_generics #where_clause {
                type Iter = #iter_type;
                pub fn #method_name(&#lifetime self) -> Self::Iter {
                    let Self ( #(#vars),* ) = self;
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
    fn test_unnamed_struct_regular_accessor() {
        let input: syn::DeriveInput = syn::parse_quote! {
            struct TestStruct<T>(SSAValue, T, SSAValue, String, f64);
        };
        let fields = extract_unnamed_field(&input);
        let info = AccessorInfo::new("arguments", "SSAValue", "::kirin_ir::HasArguments");
        let accessor = UnnamedStructRegularAccessor::scan(&info, &input, fields);
        let generated = accessor.generate();
        insta::assert_snapshot!(rustfmt(generated))
    }

    #[test]
    fn test_unnamed_struct_regular_accessor_all_matching() {
        let input: syn::DeriveInput = syn::parse_quote! {
            struct TestStruct(SSAValue, SSAValue, SSAValue);
        };
        let fields = extract_unnamed_field(&input);
        let info = AccessorInfo::new("arguments", "SSAValue", "::kirin_ir::HasArguments");
        let accessor = UnnamedStructRegularAccessor::scan(&info, &input, fields);
        let generated = accessor.generate();
        insta::assert_snapshot!(rustfmt(generated))
    }

    pub fn extract_unnamed_field(input: &syn::DeriveInput) -> &syn::FieldsUnnamed {
        match &input.data {
            syn::Data::Struct(syn::DataStruct {
                fields: syn::Fields::Unnamed(fields),
                ..
            }) => fields,
            _ => panic!("Expected unnamed struct"),
        }
    }
}
