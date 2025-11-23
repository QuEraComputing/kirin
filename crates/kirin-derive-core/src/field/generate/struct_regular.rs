use proc_macro2::TokenStream;

use crate::data::{CrateRootPath, GenerateFrom, RegularStruct, SplitForImpl, SplitForImplTrait};

use super::super::{fields::Fields, info::FieldIterInfo};

impl<'a> GenerateFrom<'a, RegularStruct<'a, FieldIterInfo>> for FieldIterInfo {
    fn generate_from(&self, data: &RegularStruct<'a, FieldIterInfo>) -> TokenStream {
        let SplitForImpl {
            impl_generics,
            input_ty_generics,
            trait_ty_generics,
            where_clause,
        } = data.split_for_impl(self);

        let name = &data.input.ident;
        let trait_path = data.absolute_path(self, &self.trait_path);

        let method_name = &self.method_name;
        let lifetime = &self.lifetime;
        let matching_type_path = &data.absolute_path(self, &self.matching_type_path);

        let item = self.item(&data.crate_root_path(self));
        let mutability = self.mutability();

        match &data.fields {
            Fields::Named(fields) => {
                let iter = fields.iterator(self.mutable, &item);
                let iter_type =
                    fields.iterator_type(self.mutable, lifetime, matching_type_path, &item);
                let unpacking_vars = fields.vars();
                quote::quote! {
                    impl #impl_generics #trait_path #trait_ty_generics for #name #input_ty_generics #where_clause {
                        type Iter = #iter_type;
                        fn #method_name(&#lifetime #mutability self) -> Self::Iter {
                            let Self { #(#unpacking_vars,)* .. } = self;
                            #iter
                        }
                    }
                }
            }
            Fields::Unnamed(fields) => {
                let iter = fields.iterator(self.mutable, &item);
                let iter_type =
                    fields.iterator_type(self.mutable, lifetime, matching_type_path, &item);
                let unpacking_vars = fields.vars();
                quote::quote! {
                    impl #impl_generics #trait_path #trait_ty_generics for #name #input_ty_generics #where_clause {
                        type Iter = #iter_type;
                        fn #method_name(&#lifetime #mutability self) -> Self::Iter {
                            let Self ( #(#unpacking_vars,)* .. ) = self;
                            #iter
                        }
                    }
                }
            }
            Fields::Unit => {
                quote::quote! {
                    impl #impl_generics #trait_path #trait_ty_generics for #name #input_ty_generics #where_clause {
                        type Iter = std::iter::Empty<#item>;
                        fn #method_name(&#lifetime #mutability self) -> Self::Iter {
                            std::iter::empty::<#item>()
                        }
                    }
                }
            }
        }
    }
}
