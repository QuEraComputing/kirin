use proc_macro2::Span;
use quote::quote;

use crate::{data::*, utils::to_snake_case};

#[macro_export]
macro_rules! derive_name {
    ($input:expr) => {{
        let trait_info = NameInfo::new();
        let data = Data::builder()
            .trait_info(&trait_info)
            .input($input)
            .build();
        trait_info.generate_from(&data)
    }};
}

pub struct NameInfo {
    trait_path: syn::Path,
    generics: syn::Generics,
}

impl NameInfo {
    pub fn new() -> Self {
        Self {
            trait_path: syn::parse_quote! { HasName },
            generics: Default::default(),
        }
    }
}

impl StatementFields<'_> for NameInfo {
    type FieldsType = ();
    type InfoType = ();
}

impl HasGenerics for NameInfo {
    fn generics(&self) -> &syn::Generics {
        &self.generics
    }
}

impl HasDefaultCratePath for NameInfo {
    fn default_crate_path(&self) -> syn::Path {
        syn::parse_quote! { ::kirin::ir }
    }
}

impl GenerateFrom<'_, NamedWrapperStruct<'_, NameInfo>> for NameInfo {
    fn generate_from(&self, data: &NamedWrapperStruct<'_, NameInfo>) -> proc_macro2::TokenStream {
        let name = &data.input.ident;
        let trait_path = data.absolute_path(self, &self.trait_path);

        let wraps = &data.wraps;
        let wraps_type = &data.wraps_type;
        let SplitForImpl {
            impl_generics,
            input_ty_generics,
            trait_ty_generics,
            where_clause,
        } = data.split_for_impl(self);

        let value = if let Some(name) = &data.attrs.name {
            quote! {
                let value = <#wraps_type as #trait_path>::name(#wraps);
                format!("{}.{}", #name, value)
            }
        } else {
            quote! {
                <#wraps_type as #trait_path>::name(#wraps)
            }
        };

        quote! {
            impl #impl_generics #trait_path #trait_ty_generics
                for #name #input_ty_generics #where_clause
            {
                fn name(&self) -> String {
                    let Self { #wraps, .. } = self;
                    #value
                }
            }
        }
    }
}

impl GenerateFrom<'_, UnnamedWrapperStruct<'_, NameInfo>> for NameInfo {
    fn generate_from(&self, data: &UnnamedWrapperStruct<'_, NameInfo>) -> proc_macro2::TokenStream {
        let name = &data.input.ident;

        let trait_path = data.absolute_path(self, &self.trait_path);
        let wraps_index = data.wraps;
        let wraps_type = &data.wraps_type;

        let vars = (0..=wraps_index)
            .map(|i| syn::Ident::new(&format!("field_{}", i), Span::call_site()))
            .collect::<Vec<_>>();
        let wraps = &vars[wraps_index];

        let SplitForImpl {
            impl_generics,
            input_ty_generics,
            trait_ty_generics,
            where_clause,
        } = data.split_for_impl(self);

        let value = if let Some(name) = &data.attrs.name {
            quote! {
                let value = <#wraps_type as #trait_path>::name(#wraps);
                format!("{}.{}", #name, value)
            }
        } else {
            quote! {
                <#wraps_type as #trait_path>::name(#wraps)
            }
        };

        quote! {
            impl #impl_generics #trait_path #trait_ty_generics
                for #name #input_ty_generics #where_clause
            {
                fn name(&self) -> String {
                    let Self (#(#vars,)* ..) = self;
                    #value
                }
            }
        }
    }
}

impl GenerateFrom<'_, RegularStruct<'_, NameInfo>> for NameInfo {
    fn generate_from(&self, data: &RegularStruct<'_, NameInfo>) -> proc_macro2::TokenStream {
        let name = &data.input.ident;
        let trait_path = data.absolute_path(self, &self.trait_path);

        let SplitForImpl {
            impl_generics,
            input_ty_generics,
            trait_ty_generics,
            where_clause,
        } = data.split_for_impl(self);

        // literal string
        let default_name: syn::LitStr = syn::LitStr::new(
            &to_snake_case(data.input.ident.to_string()),
            Span::call_site(),
        );
        let value = data
            .attrs
            .name
            .clone()
            .unwrap_or(syn::parse_quote! { #default_name });

        quote! {
            impl #impl_generics #trait_path #trait_ty_generics
                for #name #input_ty_generics #where_clause
            {
                fn name(&self) -> String {
                    #value.into()
                }
            }
        }
    }
}

macro_rules! impl_enum {
    ($kind:ident) => {
        impl GenerateFrom<'_, $kind<'_, NameInfo>> for NameInfo {
            fn generate_from(&self, data: &$kind<'_, NameInfo>) -> proc_macro2::TokenStream {
                let name = &data.input.ident;
                let trait_path = data.absolute_path(self, &self.trait_path);

                let SplitForImpl {
                    impl_generics,
                    input_ty_generics,
                    trait_ty_generics,
                    where_clause,
                } = data.split_for_impl(self);

                let default_name: syn::LitStr = syn::LitStr::new(
                    &to_snake_case(data.input.ident.to_string()),
                    Span::call_site(),
                );
                let global_default = data
                    .attrs
                    .name
                    .clone()
                    .unwrap_or(syn::parse_quote! { #default_name });
                let arms = data
                    .iter()
                    .map(|variant| variant.generate(self, &global_default, &trait_path));

                quote! {
                    impl #impl_generics #trait_path #trait_ty_generics
                        for #name #input_ty_generics #where_clause
                    {
                        fn name(&self) -> String {
                            match self {
                                #(#arms)*
                            }
                        }
                    }
                }
            }
        }
    };
}

impl_enum!(RegularEnum);
impl_enum!(EitherEnum);
impl_enum!(WrapperEnum);

trait FromVariantGenerate {
    fn generate(
        &self,
        trait_info: &NameInfo,
        global_value: &syn::Expr,
        trait_path: &syn::Path,
    ) -> proc_macro2::TokenStream;
}

impl FromVariantGenerate for RegularVariant<'_, NameInfo> {
    fn generate(
        &self,
        _trait_info: &NameInfo,
        global_value: &syn::Expr,
        _trait_path: &syn::Path,
    ) -> proc_macro2::TokenStream {
        let variant_name = &self.variant_name;
        let default_name: syn::LitStr =
            syn::LitStr::new(&to_snake_case(variant_name.to_string()), Span::call_site());
        let value = self
            .attrs
            .name
            .clone()
            .unwrap_or(syn::parse_quote! { #default_name });
        match &self.variant.fields {
            syn::Fields::Named(_) => {
                quote! {
                    Self::#variant_name { .. } => format!("{}.{}", #global_value, #value),
                }
            }
            syn::Fields::Unnamed(_) => {
                quote! {
                    Self::#variant_name ( .. ) => format!("{}.{}", #global_value, #value),
                }
            }
            syn::Fields::Unit => {
                quote! {
                    Self::#variant_name => format!("{}.{}", #global_value, #value),
                }
            }
        }
    }
}

impl FromVariantGenerate for NamedWrapperVariant<'_, NameInfo> {
    fn generate(
        &self,
        _trait_info: &NameInfo,
        global_value: &syn::Expr,
        trait_path: &syn::Path,
    ) -> proc_macro2::TokenStream {
        let variant_name = &self.variant_name;
        let wraps = &self.wraps;
        let wraps_type = &self.wraps_type;

        quote! {
            Self::#variant_name { #wraps, .. } => {
                let value = <#wraps_type as #trait_path>::name(#wraps);
                format!("{}.{}", #global_value, value)
            }
        }
    }
}

impl FromVariantGenerate for UnnamedWrapperVariant<'_, NameInfo> {
    fn generate(
        &self,
        _trait_info: &NameInfo,
        global_value: &syn::Expr,
        trait_path: &syn::Path,
    ) -> proc_macro2::TokenStream {
        let variant_name = &self.variant_name;
        let wraps_index = self.wraps;
        let wraps_type = &self.wraps_type;

        let vars = (0..=wraps_index)
            .map(|i| syn::Ident::new(&format!("field_{}", i), Span::call_site()))
            .collect::<Vec<_>>();
        let wraps = &vars[wraps_index];

        quote! {
            Self::#variant_name ( #(#vars,)* .. ) => {
                let value = <#wraps_type as #trait_path>::name(#wraps);
                format!("{}.{}", #global_value, value)
            }
        }
    }
}

impl FromVariantGenerate for WrapperVariant<'_, NameInfo> {
    fn generate(
        &self,
        trait_info: &NameInfo,
        global_value: &syn::Expr,
        trait_path: &syn::Path,
    ) -> proc_macro2::TokenStream {
        match self {
            WrapperVariant::Named(variant) => {
                variant.generate(trait_info, global_value, trait_path)
            }
            WrapperVariant::Unnamed(variant) => {
                variant.generate(trait_info, global_value, trait_path)
            }
        }
    }
}

impl FromVariantGenerate for EitherVariant<'_, NameInfo> {
    fn generate(
        &self,
        trait_info: &NameInfo,
        global_value: &syn::Expr,
        trait_path: &syn::Path,
    ) -> proc_macro2::TokenStream {
        match self {
            EitherVariant::Regular(variant) => {
                variant.generate(trait_info, global_value, trait_path)
            }
            EitherVariant::Wrapper(variant) => {
                variant.generate(trait_info, global_value, trait_path)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::rustfmt;

    #[test]
    fn test_named_wrapper_struct_name() {
        let input: syn::DeriveInput = syn::parse_quote! {
            struct TestStruct {
                #[kirin(wraps)]
                inner: InnerStruct,
            }
        };

        insta::assert_snapshot!(generate(input));
    }

    #[test]
    fn test_named_wrapper_struct_name_with_custom_name() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(name = "custom_struct")]
            struct TestStruct {
                #[kirin(wraps)]
                inner: InnerStruct,
            }
        };

        insta::assert_snapshot!(generate(input));
    }

    #[test]
    fn test_unnamed_wrapper_struct_name() {
        let input: syn::DeriveInput = syn::parse_quote! {
            struct TestStruct(#[kirin(wraps)] InnerStruct);
        };

        insta::assert_snapshot!(generate(input));
    }

    #[test]
    fn test_unnamed_wrapper_struct_name_with_custom_name() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(name = "custom_struct")]
            struct TestStruct(#[kirin(wraps)] InnerStruct);
        };

        insta::assert_snapshot!(generate(input));
    }

    #[test]
    fn test_regular_struct_name() {
        let input: syn::DeriveInput = syn::parse_quote! {
            struct TestStruct {
                field_a: SSAValue,
                field_b: i32,
            }
        };
        insta::assert_snapshot!(generate(input));
    }

    #[test]
    fn test_regular_struct_name_with_custom_name() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(name = "custom_struct")]
            struct TestStruct {
                field_a: SSAValue,
                field_b: i32,
            }
        };
        insta::assert_snapshot!(generate(input));
    }

    #[test]
    fn test_regular_enum_name() {
        let input: syn::DeriveInput = syn::parse_quote! {
            enum TestEnum {
                VariantA,
                VariantB { field: SSAValue },
                VariantC(SSAValue, i32),
            }
        };
        insta::assert_snapshot!(generate(input));
    }
    #[test]
    fn test_regular_enum_name_with_custom_name() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(name = "custom_enum")]
            enum TestEnum {
                VariantA,
                VariantB { field: SSAValue },
                VariantC(SSAValue, i32),
            }
        };
        insta::assert_snapshot!(generate(input));
    }

    #[test]
    fn test_wrapper_enum_name() {
        let input: syn::DeriveInput = syn::parse_quote! {
            enum TestEnum {
                VariantA(#[kirin(wraps)] InnerStructA),
                VariantB { #[kirin(wraps)] inner: InnerStructB },
            }
        };
        insta::assert_snapshot!(generate(input));
    }

    #[test]
    fn test_wrapper_enum_name_with_custom_name() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(name = "custom_enum")]
            enum TestEnum {
                VariantA(#[kirin(wraps)] InnerStructA),
                VariantB { #[kirin(wraps)] inner: InnerStructB },
            }
        };
        insta::assert_snapshot!(generate(input));
    }

    #[test]
    fn test_either_enum_name() {
        let input: syn::DeriveInput = syn::parse_quote! {
            enum TestEnum {
                VariantA(#[kirin(wraps)] InnerStructA),
                VariantB { field: SSAValue },
            }
        };
        insta::assert_snapshot!(generate(input));
    }

    #[test]
    fn test_either_enum_name_with_custom_name() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(name = "custom_enum")]
            enum TestEnum {
                VariantA(#[kirin(wraps)] InnerStructA),
                VariantB { field: SSAValue },
            }
        };
        insta::assert_snapshot!(generate(input));
    }

    fn generate(input: syn::DeriveInput) -> String {
        rustfmt(derive_name!(&input))
    }
}
