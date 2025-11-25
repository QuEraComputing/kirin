use crate::data::*;
use proc_macro2::Span;
use quote::{format_ident, quote};

#[macro_export]
macro_rules! derive_check {
    ($input:expr, $method_name:ident, $trait_path:expr) => {{
        CheckInfo::new(
            stringify!($method_name),
            syn::parse_quote! {$trait_path},
            |attr: &dyn PropertyAttribute| attr.$method_name(),
        )
        .and_then(|trait_info| {
            let data = Data::builder()
                .trait_info(&trait_info)
                .input($input)
                .build();
            Ok(trait_info.generate_from(&data))
        })
        .unwrap_or_else(|e| e.to_compile_error())
    }};
}

pub struct CheckInfo {
    pub f: fn(&dyn PropertyAttribute) -> Option<bool>,
    pub method_name: syn::Ident,
    pub trait_path: syn::Path,
    generics: syn::Generics,
}

impl CheckInfo {
    pub fn new(
        method_name: impl AsRef<str>,
        trait_path: syn::Path,
        f: fn(&dyn PropertyAttribute) -> Option<bool>,
    ) -> syn::Result<Self> {
        Ok(Self {
            f,
            method_name: format_ident!("{}", method_name.as_ref()),
            trait_path,
            generics: syn::Generics::default(),
        })
    }
}

impl StatementFields<'_> for CheckInfo {
    type FieldsType = Option<bool>;
    type InfoType = Option<bool>;
}

impl HasGenerics for CheckInfo {
    fn generics(&self) -> &syn::Generics {
        &self.generics
    }
}

impl HasDefaultCratePath for CheckInfo {
    fn default_crate_path(&self) -> syn::Path {
        syn::parse_quote! { ::kirin::ir }
    }
}

impl FromStruct<'_, CheckInfo> for Option<bool> {
    fn from_struct(
        trait_info: &CheckInfo,
        attrs: &StructAttribute,
        _input: &'_ syn::DeriveInput,
    ) -> syn::Result<Self> {
        Ok((trait_info.f)(attrs))
    }
}

impl FromEnum<'_, CheckInfo> for Option<bool> {
    fn from_enum(
        trait_info: &CheckInfo,
        attrs: &EnumAttribute,
        _input: &'_ syn::DeriveInput,
    ) -> syn::Result<Self> {
        Ok((trait_info.f)(attrs))
    }
}

impl FromStructFields<'_, CheckInfo> for Option<bool> {
    fn from_struct_fields(
        trait_info: &CheckInfo,
        attrs: &StructAttribute,
        _parent: &'_ syn::DataStruct,
        _fields: &'_ syn::Fields,
    ) -> syn::Result<Self> {
        Ok((trait_info.f)(attrs))
    }
}

impl FromVariantFields<'_, CheckInfo> for Option<bool> {
    fn from_variant_fields(
        trait_info: &CheckInfo,
        attrs: &VariantAttribute,
        _parent: &'_ syn::Variant,
        _fields: &'_ syn::Fields,
    ) -> syn::Result<Self> {
        Ok((trait_info.f)(attrs))
    }
}

impl GenerateFrom<'_, NamedWrapperStruct<'_, CheckInfo>> for CheckInfo {
    fn generate_from(&self, data: &NamedWrapperStruct<'_, CheckInfo>) -> proc_macro2::TokenStream {
        let name = &data.input.ident;
        let method_name = &self.method_name;
        let trait_path = data.absolute_path(self, &self.trait_path);
        let wraps = &data.wraps;
        let wraps_type = &data.wraps_type;
        let SplitForImpl {
            impl_generics,
            input_ty_generics,
            trait_ty_generics,
            where_clause,
        } = data.split_for_impl(self);

        quote! {
            impl #impl_generics #trait_path #trait_ty_generics
                for #name #input_ty_generics #where_clause
            {
                fn #method_name(&self) -> bool {
                    let Self { #wraps, .. } = self;
                    <#wraps_type as #trait_path>::#method_name(#wraps)
                }
            }
        }
    }
}

impl GenerateFrom<'_, UnnamedWrapperStruct<'_, CheckInfo>> for CheckInfo {
    fn generate_from(
        &self,
        data: &UnnamedWrapperStruct<'_, CheckInfo>,
    ) -> proc_macro2::TokenStream {
        let name = &data.input.ident;
        let method_name = &self.method_name;
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

        quote! {
            impl #impl_generics #trait_path #trait_ty_generics
                for #name #input_ty_generics #where_clause
            {
                fn #method_name(&self) -> bool {
                    let Self (#(#vars,)* ..) = self;
                    <#wraps_type as #trait_path>::#method_name(#wraps)
                }
            }
        }
    }
}

impl GenerateFrom<'_, RegularStruct<'_, CheckInfo>> for CheckInfo {
    fn generate_from(&self, data: &RegularStruct<'_, CheckInfo>) -> proc_macro2::TokenStream {
        let name = &data.input.ident;
        let method_name = &self.method_name;
        let trait_path = data.absolute_path(self, &self.trait_path);

        let SplitForImpl {
            impl_generics,
            input_ty_generics,
            trait_ty_generics,
            where_clause,
        } = data.split_for_impl(self);

        let value = data.struct_info.unwrap_or(false) || data.fields.unwrap_or(false);

        quote! {
            impl #impl_generics #trait_path #trait_ty_generics
                for #name #input_ty_generics #where_clause
            {
                fn #method_name(&self) -> bool {
                    #value
                }
            }
        }
    }
}

macro_rules! impl_enum {
    ($kind:ident) => {
        impl GenerateFrom<'_, $kind<'_, CheckInfo>> for CheckInfo {
            fn generate_from(&self, data: &$kind<'_, CheckInfo>) -> proc_macro2::TokenStream {
                let name = &data.input.ident;
                let method_name = &self.method_name;
                let trait_path = data.absolute_path(self, &self.trait_path);

                let SplitForImpl {
                    impl_generics,
                    input_ty_generics,
                    trait_ty_generics,
                    where_clause,
                } = data.split_for_impl(self);

                let global_default = data.enum_info.unwrap_or(false);
                let arms = data
                    .iter()
                    .map(|variant| variant.generate(self, global_default, &trait_path));

                quote! {
                    impl #impl_generics #trait_path #trait_ty_generics
                        for #name #input_ty_generics #where_clause
                    {
                        fn #method_name(&self) -> bool {
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

impl GenerateFrom<'_, WrapperEnum<'_, CheckInfo>> for CheckInfo {
    fn generate_from(&self, data: &WrapperEnum<'_, CheckInfo>) -> proc_macro2::TokenStream {
        let name = &data.input.ident;
        let method_name = &self.method_name;
        let trait_path = data.absolute_path(self, &self.trait_path);

        let SplitForImpl {
            impl_generics,
            input_ty_generics,
            trait_ty_generics,
            where_clause,
        } = data.split_for_impl(self);

        let arms = data
            .iter()
            .map(|variant| variant.generate(self, false, &trait_path));

        quote! {
            impl #impl_generics #trait_path #trait_ty_generics
                for #name #input_ty_generics #where_clause
            {
                fn #method_name(&self) -> bool {
                    match self {
                        #(#arms)*
                    }
                }
            }
        }
    }
}

trait FromVariantGenerate {
    fn generate(
        &self,
        trait_info: &CheckInfo,
        global_value: bool,
        trait_path: &syn::Path,
    ) -> proc_macro2::TokenStream;
}

impl FromVariantGenerate for RegularVariant<'_, CheckInfo> {
    fn generate(
        &self,
        _trait_info: &CheckInfo,
        global_value: bool,
        _trait_path: &syn::Path,
    ) -> proc_macro2::TokenStream {
        let variant_name = &self.variant_name;
        let value = global_value || self.fields.unwrap_or(false);
        match &self.variant.fields {
            syn::Fields::Named(_) => {
                quote! {
                    Self::#variant_name { .. } => #value,
                }
            }
            syn::Fields::Unnamed(_) => {
                quote! {
                    Self::#variant_name ( .. ) => #value,
                }
            }
            syn::Fields::Unit => {
                quote! {
                    Self::#variant_name => #value,
                }
            }
        }
    }
}

impl FromVariantGenerate for NamedWrapperVariant<'_, CheckInfo> {
    fn generate(
        &self,
        trait_info: &CheckInfo,
        _global_value: bool,
        trait_path: &syn::Path,
    ) -> proc_macro2::TokenStream {
        let variant_name = &self.variant_name;
        let method_name = &trait_info.method_name;
        let wraps = &self.wraps;
        let wraps_type = &self.wraps_type;

        quote! {
            Self::#variant_name { #wraps, .. } => {
                <#wraps_type as #trait_path>::#method_name(#wraps)
            }
        }
    }
}

impl FromVariantGenerate for UnnamedWrapperVariant<'_, CheckInfo> {
    fn generate(
        &self,
        trait_info: &CheckInfo,
        _global_value: bool,
        trait_path: &syn::Path,
    ) -> proc_macro2::TokenStream {
        let variant_name = &self.variant_name;
        let method_name = &trait_info.method_name;
        let wraps_index = self.wraps;
        let wraps_type = &self.wraps_type;

        let vars = (0..=wraps_index)
            .map(|i| syn::Ident::new(&format!("field_{}", i), Span::call_site()))
            .collect::<Vec<_>>();
        let wraps = &vars[wraps_index];

        quote! {
            Self::#variant_name ( #(#vars,)* .. ) => {
                <#wraps_type as #trait_path>::#method_name(#wraps)
            }
        }
    }
}

impl FromVariantGenerate for WrapperVariant<'_, CheckInfo> {
    fn generate(
        &self,
        trait_info: &CheckInfo,
        global_value: bool,
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

impl FromVariantGenerate for EitherVariant<'_, CheckInfo> {
    fn generate(
        &self,
        trait_info: &CheckInfo,
        global_value: bool,
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
    fn test_struct_global_wrapper() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(wraps)]
            struct WrapperStruct<T> (T);
        };
        insta::assert_snapshot!(generate(input));

        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(wraps)]
            struct WrapperStruct<T> {
                field: T,
            }
        };
        insta::assert_snapshot!(generate(input));
    }

    #[test]
    fn test_struct_field_wrapper() {
        let input: syn::DeriveInput = syn::parse_quote! {
            struct WrapperStruct<T> {
                #[kirin(wraps)]
                field: T,
                other: u32,
            }
        };
        insta::assert_snapshot!(generate(input));
    }

    #[test]
    fn test_enum_wrapper() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(wraps)]
            enum WrapperEnum<T> {
                Variant1(T),
                Variant2 { field: T },
                Variant3(T, #[kirin(wraps)] Other),
            }
        };
        insta::assert_snapshot!(generate(input));
    }

    fn generate(input: syn::DeriveInput) -> String {
        rustfmt(derive_check!(&input, is_constant, IsConstant))
    }
}
