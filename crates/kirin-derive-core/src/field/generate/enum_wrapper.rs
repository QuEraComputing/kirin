use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use super::super::info::FieldIterInfo;
use super::{iter_variant::IteratorVariant, method_arm::MethodArm, next_arm::IteratorNextArm};
use crate::data::*;

impl GenerateFrom<'_, WrapperEnum<'_, FieldIterInfo>> for FieldIterInfo {
    fn generate_from(&self, data: &WrapperEnum<'_, FieldIterInfo>) -> TokenStream {
        let SplitForImpl {
            impl_generics,
            input_ty_generics,
            trait_ty_generics,
            where_clause,
        } = data.split_for_impl(self);
        let name = &data.input.ident;

        let trait_path = data.absolute_path(self, &self.trait_path);
        let matching_type_path = &data.absolute_path(self, &self.matching_type_path);

        let iter_name = format_ident!("{}{}", name, &self.iter_name);
        let method_name = &self.method_name;
        let lifetime = &self.lifetime;

        let mutability = self.mutability();
        let item = self.item(&data.crate_root_path(self));

        let iter_variants = data.variants.iter().map(|variant| {
            variant.generate_iterator_variant(
                &self,
                &trait_path,
                &trait_ty_generics,
                &matching_type_path,
                &item,
            )
        });
        let method_arms = data.variants.iter().map(|variant| {
            variant.generate_method_arm(
                self.mutable,
                name,
                &iter_name,
                &trait_path,
                method_name,
                &item,
            )
        });
        let iter_next_arms = data
            .variants
            .iter()
            .map(|variant| variant.generate_iterator_next_arm(&iter_name));

        let mut iter_generics = self.generics.clone(); // <'a>
        iter_generics
            .params
            .extend(data.input.generics.params.iter().cloned()); // <'a, T, U, ...>
        let (iter_impl_generics, iter_ty_generics, iter_where_clause) =
            iter_generics.split_for_impl();

        quote! {
            impl #impl_generics #trait_path #trait_ty_generics for #name #input_ty_generics #where_clause {
                type Iter = #iter_name<#lifetime>;
                fn #method_name(&#lifetime #mutability self) -> Self::Iter {
                    match self {
                        #(#method_arms)*
                    }
                }
            }

            pub enum #iter_name #iter_generics {
                #(#iter_variants),*
            }

            impl #iter_impl_generics Iterator for #iter_name #iter_ty_generics #iter_where_clause {
                type Item = #item;
                fn next(&mut self) -> Option<Self::Item> {
                    match self {
                        #(#iter_next_arms)*
                    }
                }
            }
        }
    }
}
