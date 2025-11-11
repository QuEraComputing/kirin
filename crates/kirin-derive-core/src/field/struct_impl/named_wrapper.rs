use proc_macro2::TokenStream;
use quote::quote;

use crate::field::data::AccessorInfo;

pub struct NamedStructWrapperAccessor<'input> {
    pub info: &'input AccessorInfo,
    pub name: &'input syn::Ident,
    pub generics: &'input syn::Generics,
    pub wraps: syn::Ident,
    pub wraps_type: syn::Type,
}

impl<'input> NamedStructWrapperAccessor<'input> {
    pub fn generate(&self) -> TokenStream {
        let name = &self.name;
        let wraps = &self.wraps;
        let wraps_type = &self.wraps_type;
        let info = &self.info;
        let method_name = &self.info.name;
        let trait_path = &info.trait_path;
        let generics = self.info.generics(&self.generics);
        let (input_type_generics, lifetime, impl_generics, _, where_clause) =
            generics.split_for_impl();

        quote! {
            impl #impl_generics #trait_path<#lifetime> for #name #input_type_generics #where_clause {
                type Iter = <#wraps_type as #trait_path>::Iter;
                pub fn #method_name(&#lifetime self) -> Self::Iter {
                    let Self { #wraps, .. } = self;
                    <#wraps_type as #trait_path>::#method_name(#wraps)
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
    fn test_named_struct_wrapper_accessor() {
        let input: syn::DeriveInput = syn::parse_quote! {
            struct TestStruct<T> {
                wrapped: InnerStruct<T>,
            }
        };
        let fields = extract_named_field(&input);
        let info = AccessorInfo::new("arguments", "SSAValue", "::kirin_ir::HasArguments");
        let accessor = NamedStructWrapperAccessor {
            info: &info,
            name: &input.ident,
            generics: &input.generics,
            wraps: fields.named.first().unwrap().ident.clone().unwrap(),
            wraps_type: fields.named.first().unwrap().ty.clone(),
        };
        let generated = accessor.generate();
        insta::assert_snapshot!(rustfmt(generated))
    }

    pub fn extract_named_field(input: &syn::DeriveInput) -> &syn::FieldsNamed {
        match &input.data {
            syn::Data::Struct(syn::DataStruct {
                fields: syn::Fields::Named(fields),
                ..
            }) => fields,
            _ => panic!("expected named fields"),
        }
    }
}
