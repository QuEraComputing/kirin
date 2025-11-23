use proc_macro2::Span;
use quote::quote;

use super::super::info::FieldIterInfo;
use crate::data::{
    CrateRootPath, GenerateFrom, NamedWrapperStruct, SplitForImpl, SplitForImplTrait,
    UnnamedWrapperStruct,
};

impl GenerateFrom<'_, NamedWrapperStruct<'_, FieldIterInfo>> for FieldIterInfo {
    fn generate_from(&self, data: &NamedWrapperStruct<'_, FieldIterInfo>) -> proc_macro2::TokenStream {
        let SplitForImpl {
            impl_generics,
            input_ty_generics,
            trait_ty_generics,
            where_clause,
        } = data.split_for_impl(self);

        let name = &data.input.ident;
        let trait_path = data.absolute_path(self, &self.trait_path);

        let method_name = &self.method_name;
        let wraps = &data.wraps;
        let wraps_type = &data.wraps_type;
        let lifetime = &self.lifetime;

        let mutability = self.mutability();

        quote! {
            impl #impl_generics #trait_path #trait_ty_generics for #name #input_ty_generics #where_clause {
                type Iter = <#wraps_type as #trait_path>::Iter;
                fn #method_name(&#lifetime #mutability self) -> Self::Iter {
                    let Self { #wraps, .. } = self;
                    <#wraps_type as #trait_path>::#method_name(#wraps)
                }
            }
        }
    }
}

impl GenerateFrom<'_, UnnamedWrapperStruct<'_, FieldIterInfo>> for FieldIterInfo {
    fn generate_from(
        &self,
        data: &UnnamedWrapperStruct<'_, FieldIterInfo>,
    ) -> proc_macro2::TokenStream {
        let SplitForImpl {
            impl_generics,
            input_ty_generics,
            trait_ty_generics,
            where_clause,
        } = data.split_for_impl(self);

        let name = &data.input.ident;
        let trait_path = data.absolute_path(self, &self.trait_path);

        let method_name = &self.method_name;
        let wraps_index = data.wraps;
        let vars = (0..=wraps_index)
            .map(|i| syn::Ident::new(&format!("field_{}", i), Span::call_site()))
            .collect::<Vec<syn::Ident>>();
        let wraps = &vars[wraps_index];
        let wraps_type = &data.wraps_type;
        let lifetime = &self.lifetime;

        let mutability = self.mutability();

        quote! {
            impl #impl_generics #trait_path #trait_ty_generics for #name #input_ty_generics #where_clause {
                type Iter = <#wraps_type as #trait_path>::Iter;
                fn #method_name(&#lifetime #mutability self) -> Self::Iter {
                    let Self( #(#vars,)* .. ) = self;
                    <#wraps_type as #trait_path>::#method_name(#wraps)
                }
            }
        }
    }
}
