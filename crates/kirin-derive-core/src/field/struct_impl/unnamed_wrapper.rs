use proc_macro2::{Span, TokenStream};
use quote::quote;

use crate::field::data::AccessorInfo;

pub struct UnnamedStructWrapperAccessor<'input> {
    pub info: &'input AccessorInfo,
    /// The name of the struct
    pub name: &'input syn::Ident,
    /// The generics of the struct
    pub generics: &'input syn::Generics,
    /// The index of the field being wrapped
    pub wraps: usize,
    /// The type of the field being wrapped
    pub wraps_type: syn::Type,
}

impl<'input> UnnamedStructWrapperAccessor<'input> {
    pub fn generate(&self) -> TokenStream {
        let name = &self.name;
        let wraps_type = &self.wraps_type;
        let info = &self.info;
        let trait_path = &info.trait_path;
        let method_name = &self.info.name;
        let vars = (0..=self.wraps)
            .map(|i| syn::Ident::new(&format!("field_{}", i), Span::call_site()))
            .collect::<Vec<_>>();
        let wraps_name = &vars[self.wraps];
        let generics = self.info.generics(&self.generics);
        let (input_type_generics, lifetime, impl_generics, _, where_clause) =
            generics.split_for_impl();

        quote! {
            impl #impl_generics #trait_path<#lifetime> for #name #input_type_generics #where_clause {
                type Iter = <#wraps_type as #trait_path>::Iter;
                pub fn #method_name(&#lifetime self) -> Self::Iter {
                    let Self (#(#vars,)* ..) = self;
                    <#wraps_type as #trait_path>::#method_name(#wraps_name)
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
    fn test_unnamed_struct_wrapper_accessor() {
        let input: syn::DeriveInput = syn::parse_quote! {
            struct TestStruct<T>(InnerStruct, T, String);
        };
        let fields = extract_unnamed_field(&input);
        let info = AccessorInfo::new("arguments", "SSAValue", "::kirin_ir::HasArguments");
        let accessor = UnnamedStructWrapperAccessor {
            info: &info,
            name: &input.ident,
            generics: &input.generics,
            wraps: 0,
            wraps_type: fields.unnamed.first().unwrap().ty.clone(),
        };
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