use quote::quote;

use crate::data::*;

#[macro_export]
macro_rules! derive_from {
    ($input:expr) => {{
        let trait_info = FromInfo::default();
        let data = Data::builder()
            .trait_info(&trait_info)
            .input($input)
            .build();
        trait_info.generate_from(&data)
    }};
}

#[derive(Clone, Default)]
pub struct FromInfo(syn::Generics);

impl StatementFields<'_> for FromInfo {
    type FieldsType = ();
    type InfoType = ();
}

impl HasGenerics for FromInfo {
    fn generics(&self) -> &syn::Generics {
        &self.0
    }
}

impl GenerateFrom<'_, NamedWrapperStruct<'_, FromInfo>> for FromInfo {
    fn generate_from(&self, data: &NamedWrapperStruct<'_, FromInfo>) -> proc_macro2::TokenStream {
        let name = &data.input.ident;
        let wraps = &data.wraps;
        let wraps_type = &data.wraps_type;

        let SplitForImpl {
            impl_generics,
            trait_ty_generics: _,
            input_ty_generics,
            where_clause,
        } = data.split_for_impl(&self);

        let syn::Data::Struct(data) = &data.input.data else {
            panic!("GenerateFrom for FromTraitInfo only supports structs");
        };

        let initialization = data
            .fields
            .iter()
            .map(|f| {
                if let Some(name) = &f.ident {
                    if name == wraps {
                        quote! { #wraps: v }
                    } else {
                        quote! { #name: Default::default() }
                    }
                } else {
                    panic!("GenerateFrom for FromTraitInfo only supports named fields");
                }
            })
            .collect::<Vec<_>>();

        quote! {
            impl #impl_generics ::core::convert::From<#wraps_type>
                for #name #input_ty_generics
                #where_clause
            {
                fn from(v: #wraps_type) -> Self {
                    Self { #(#initialization),* }
                }
            }
        }
    }
}

impl GenerateFrom<'_, UnnamedWrapperStruct<'_, FromInfo>> for FromInfo {
    fn generate_from(&self, data: &UnnamedWrapperStruct<'_, FromInfo>) -> proc_macro2::TokenStream {
        let name = &data.input.ident;
        let wraps = &data.wraps;
        let wraps_type = &data.wraps_type;

        let SplitForImpl {
            impl_generics,
            trait_ty_generics: _,
            input_ty_generics,
            where_clause,
        } = data.split_for_impl(&self);

        let syn::Data::Struct(data) = &data.input.data else {
            panic!("GenerateFrom for FromTraitInfo only supports structs");
        };

        let initialization = data
            .fields
            .iter()
            .enumerate()
            .map(|(i, _)| {
                if i == *wraps {
                    quote! { v }
                } else {
                    quote! { Default::default() }
                }
            })
            .collect::<Vec<_>>();

        quote! {
            impl #impl_generics ::core::convert::From<#wraps_type>
                for #name #input_ty_generics
                #where_clause
            {
                fn from(v: #wraps_type) -> Self {
                    Self(#(#initialization),*)
                }
            }
        }
    }
}

impl GenerateFrom<'_, WrapperEnum<'_, FromInfo>> for FromInfo {
    fn generate_from(&self, data: &WrapperEnum<'_, FromInfo>) -> proc_macro2::TokenStream {
        let SplitForImpl {
            impl_generics,
            trait_ty_generics: _,
            input_ty_generics,
            where_clause,
        } = data.split_for_impl(&self);

        let variants = data
            .variants
            .iter()
            .map(|variant| {
                let method = self.generate_from(variant);
                let enum_name = &data.input.ident;
                let wraps_type = &variant.wraps_type();
                quote! {
                    impl #impl_generics From<#wraps_type> for #enum_name #input_ty_generics #where_clause {
                        #method
                    }
                }
            })
            .collect::<Vec<_>>();

        quote! {
            #(#variants)*
        }
    }
}

impl GenerateFrom<'_, EitherEnum<'_, FromInfo>> for FromInfo {
    fn generate_from(&self, data: &EitherEnum<'_, FromInfo>) -> proc_macro2::TokenStream {
        let SplitForImpl {
            impl_generics,
            trait_ty_generics: _,
            input_ty_generics,
            where_clause,
        } = data.split_for_impl(&self);

        let variants = data
            .variants
            .iter()
            .map(|variant| {
                let method = self.generate_from(variant);
                match variant {
                    EitherVariant::Wrapper(variant) => {
                        let enum_name = &data.input.ident;
                        let wraps_type = &variant.wraps_type();
                        quote! {
                            impl #impl_generics From<#wraps_type> for #enum_name #input_ty_generics #where_clause {
                                #method
                            }
                        }
                    }
                    _ => quote! {},
                }
            })
            .collect::<Vec<_>>();
        quote! {
            #(#variants)*
        }
    }
}

impl GenerateFrom<'_, EitherVariant<'_, FromInfo>> for FromInfo {
    fn generate_from(&self, data: &EitherVariant<'_, FromInfo>) -> proc_macro2::TokenStream {
        match data {
            EitherVariant::Wrapper(v) => self.generate_from(v),
            EitherVariant::Regular(v) => self.generate_from(v),
        }
    }
}

impl GenerateFrom<'_, WrapperVariant<'_, FromInfo>> for FromInfo {
    fn generate_from(&self, data: &WrapperVariant<'_, FromInfo>) -> proc_macro2::TokenStream {
        match data {
            WrapperVariant::Named(data) => self.generate_from(data),
            WrapperVariant::Unnamed(data) => self.generate_from(data),
        }
    }
}

impl GenerateFrom<'_, NamedWrapperVariant<'_, FromInfo>> for FromInfo {
    fn generate_from(&self, data: &NamedWrapperVariant<'_, FromInfo>) -> proc_macro2::TokenStream {
        let wraps = &data.wraps;
        let wraps_type = &data.wraps_type;
        let variant_name = &data.variant_name;

        let initialization = data
            .variant
            .fields
            .iter()
            .map(|f| {
                if let Some(name) = &f.ident {
                    if name == wraps {
                        quote! { #wraps: v }
                    } else {
                        quote! { #name: Default::default() }
                    }
                } else {
                    panic!("GenerateFrom for FromTraitInfo only supports named fields");
                }
            })
            .collect::<Vec<_>>();

        quote! {
            fn from(v: #wraps_type) -> Self {
                Self::#variant_name { #(#initialization),* }
            }
        }
    }
}

impl GenerateFrom<'_, UnnamedWrapperVariant<'_, FromInfo>> for FromInfo {
    fn generate_from(
        &self,
        data: &UnnamedWrapperVariant<'_, FromInfo>,
    ) -> proc_macro2::TokenStream {
        let wraps = &data.wraps;
        let wraps_type = &data.wraps_type;
        let variant_name = &data.variant_name;

        let initialization = data
            .variant
            .fields
            .iter()
            .enumerate()
            .map(|(i, _)| {
                if i == *wraps {
                    quote! { v }
                } else {
                    quote! { Default::default() }
                }
            })
            .collect::<Vec<_>>();

        quote! {
            fn from(v: #wraps_type) -> Self {
                Self::#variant_name(#(#initialization),*)
            }
        }
    }
}

impl GenerateFrom<'_, RegularStruct<'_, FromInfo>> for FromInfo {
    fn generate_from(&self, _data: &RegularStruct<'_, FromInfo>) -> proc_macro2::TokenStream {
        quote! {}
    }
}

impl GenerateFrom<'_, RegularEnum<'_, FromInfo>> for FromInfo {
    fn generate_from(&self, _data: &RegularEnum<'_, FromInfo>) -> proc_macro2::TokenStream {
        quote! {}
    }
}

impl GenerateFrom<'_, RegularVariant<'_, FromInfo>> for FromInfo {
    fn generate_from(&self, _data: &RegularVariant<'_, FromInfo>) -> proc_macro2::TokenStream {
        quote! {}
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::rustfmt;

    #[test]
    fn test_generate_from_named_wrapper_struct() {
        let input: syn::DeriveInput = syn::parse_quote! {
            struct WrapperStruct {
                a: i32,
                #[kirin(wraps)]
                b: Other,
                c: f64,
            }
        };
        insta::assert_snapshot!(generate(input));
    }

    #[test]
    fn test_generate_from_unnamed_wrapper_struct() {
        let input: syn::DeriveInput = syn::parse_quote! {
            struct WrapperStruct(i32, #[kirin(wraps)] Other, f64);
        };
        insta::assert_snapshot!(generate(input));
    }

    #[test]
    fn test_generate_from_wrapper_enum() {
        let input: syn::DeriveInput = syn::parse_quote! {
            enum WrapperEnum<T> {
                Variant1(#[kirin(wraps)] A),
                Variant2 { #[kirin(wraps)] field: B },
            }
        };
        insta::assert_snapshot!(generate(input));
    }

    #[test]
    fn test_generate_from_either_enum() {
        let input: syn::DeriveInput = syn::parse_quote! {
            enum EitherEnum<T> {
                WrapperVariant1(#[kirin(wraps)] A),
                RegularVariant2 { field: B },
            }
        };
        insta::assert_snapshot!(generate(input));
    }

    #[test]
    fn test_generate_from_regular_struct() {
        let input: syn::DeriveInput = syn::parse_quote! {
            struct RegularStruct {
                a: i32,
                b: f64,
            }
        };
        insta::assert_snapshot!(generate(input));
    }

    #[test]
    fn test_generate_from_regular_enum() {
        let input: syn::DeriveInput = syn::parse_quote! {
            enum RegularEnum {
                Variant1(i32),
                Variant2 { field: f64 },
            }
        };
        insta::assert_snapshot!(generate(input));
    }

    fn generate(input: syn::DeriveInput) -> String {
        rustfmt(derive_from!(&input))
    }
}
