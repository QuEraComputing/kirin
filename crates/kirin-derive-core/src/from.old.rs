use quote::quote;

use crate::data::*;

#[macro_export]
macro_rules! derive_from {
    ($input:expr) => {{
        let ctx = Context::new(FromTraitInfo::default(), $input);
        let data = DataTrait::new(&ctx);
        ctx.generate_from(&data)
    }};
}

pub struct FromTraitInfo {
    method_name: syn::Ident,
    trait_path: syn::Path,
    generics: syn::Generics,
}

impl Default for FromTraitInfo {
    fn default() -> Self {
        FromTraitInfo {
            method_name: syn::Ident::new("from", proc_macro2::Span::call_site()),
            trait_path: syn::parse_quote! { From },
            generics: syn::Generics::default(),
        }
    }
}

impl<'input> TraitInfo<'input> for FromTraitInfo {
    type GlobalAttributeData = ();
    type MatchingFields = ();
    fn method_name(&self) -> &syn::Ident {
        &self.method_name
    }

    fn default_crate_path(&self) -> syn::Path {
        syn::parse_quote! { ::core::convert }
    }

    fn relative_trait_path(&self) -> &syn::Path {
        &self.trait_path
    }

    fn trait_generics(&self) -> &syn::Generics {
        &self.generics
    }
}

impl FromStructFields<'_, FromTraitInfo> for () {
    fn from_struct_fields(
        _ctx: &crate::data::Context<'_, FromTraitInfo>,
        _parent: &syn::DataStruct,
        _fields: &syn::Fields,
    ) -> Self {
        ()
    }
}

impl FromVariantFields<'_, FromTraitInfo> for () {
    fn from_variant_fields(
        _ctx: &crate::data::Context<'_, FromTraitInfo>,
        _parent: &syn::Variant,
        _fields: &syn::Fields,
    ) -> Self {
        ()
    }
}

impl GenerateFrom<'_, NamedWrapperStruct<'_, FromTraitInfo>> for FromTraitInfo {
    fn generate_from(
        &self,
        data: &NamedWrapperStruct<'_, FromTraitInfo>,
    ) -> proc_macro2::TokenStream {
        let name = &data.ctx.input.ident;
        let wraps = &data.wraps;
        let wraps_type = &data.wraps_type;
        let (impl_generics, ty_generics, where_clause) = data.ctx.input.generics.split_for_impl();

        let syn::Data::Struct(data) = &data.ctx.input.data else {
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
            impl #impl_generics From<#wraps_type> for #name #ty_generics #where_clause {
                fn from(v: #wraps_type) -> Self {
                    Self { #(#initialization),* }
                }
            }
        }
    }
}

impl GenerateFrom<'_, UnnamedWrapperStruct<'_, FromTraitInfo>> for FromTraitInfo {
    fn generate_from(
        &self,
        data: &UnnamedWrapperStruct<'_, FromTraitInfo>,
    ) -> proc_macro2::TokenStream {
        let name = &data.ctx.input.ident;
        let wraps = &data.wraps;
        let wraps_type = &data.wraps_type;
        let (impl_generics, ty_generics, where_clause) = data.ctx.input.generics.split_for_impl();

        let syn::Data::Struct(data) = &data.ctx.input.data else {
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
            impl #impl_generics From<#wraps_type> for #name #ty_generics #where_clause {
                fn from(v: #wraps_type) -> Self {
                    Self(#(#initialization),*)
                }
            }
        }
    }
}

impl GenerateFrom<'_, WrapperEnum<'_, FromTraitInfo>> for FromTraitInfo {
    fn generate_from(&self, data: &WrapperEnum<'_, FromTraitInfo>) -> proc_macro2::TokenStream {
        let variants = data
            .variants
            .iter()
            .map(|variant| self.generate_from(variant))
            .collect::<Vec<_>>();

        quote! {
            #(#variants)*
        }
    }
}

impl GenerateFrom<'_, EitherEnum<'_, FromTraitInfo>> for FromTraitInfo {
    fn generate_from(&self, data: &EitherEnum<'_, FromTraitInfo>) -> proc_macro2::TokenStream {
        let variants = data
            .variants
            .iter()
            .map(|variant| self.generate_from(variant))
            .collect::<Vec<_>>();
        quote! {
            #(#variants)*
        }
    }
}

impl GenerateFrom<'_, EitherVariant<'_, FromTraitInfo>> for FromTraitInfo {
    fn generate_from(
        &self,
        data: &EitherVariant<'_, FromTraitInfo>,
    ) -> proc_macro2::TokenStream {
        match data {
            EitherVariant::Wrapper(data) => self.generate_from(data),
            EitherVariant::Regular(data) => self.generate_from(data),
        }
    }
}

impl GenerateFrom<'_, WrapperVariant<'_, FromTraitInfo>> for FromTraitInfo {
    fn generate_from(&self, data: &WrapperVariant<'_, FromTraitInfo>) -> proc_macro2::TokenStream {
        match data {
            WrapperVariant::Named(data) => self.generate_from(data),
            WrapperVariant::Unnamed(data) => self.generate_from(data),
        }
    }
}

impl GenerateFrom<'_, NamedWrapperVariant<'_, FromTraitInfo>> for FromTraitInfo {
    fn generate_from(
        &self,
        data: &NamedWrapperVariant<'_, FromTraitInfo>,
    ) -> proc_macro2::TokenStream {
        let enum_name = &data.ctx.input.ident;
        let variant_name = &data.variant_name;
        let wraps = &data.wraps;
        let wraps_type = &data.wraps_type;
        let (impl_generics, ty_generics, where_clause) = data.ctx.input.generics.split_for_impl();
        let syn::Data::Enum(data) = &data.ctx.input.data else {
            panic!("GenerateFrom for FromTraitInfo only supports enums");
        };
        let variant = data
            .variants
            .iter()
            .find(|v| &v.ident == *variant_name)
            .expect("Variant not found");
        let initialization = variant
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
            impl #impl_generics From<#wraps_type> for #enum_name #ty_generics #where_clause {
                fn from(v: #wraps_type) -> Self {
                    Self::#variant_name { #(#initialization),* }
                }
            }
        }
    }
}

impl GenerateFrom<'_, UnnamedWrapperVariant<'_, FromTraitInfo>> for FromTraitInfo {
    fn generate_from(
        &self,
        data: &UnnamedWrapperVariant<'_, FromTraitInfo>,
    ) -> proc_macro2::TokenStream {
        let enum_name = &data.ctx.input.ident;
        let variant_name = &data.variant_name;
        let wraps = &data.wraps;
        let wraps_type = &data.wraps_type;
        let (impl_generics, ty_generics, where_clause) = data.ctx.input.generics.split_for_impl();
        let syn::Data::Enum(data) = &data.ctx.input.data else {
            panic!("GenerateFrom for FromTraitInfo only supports enums");
        };
        let variant = data
            .variants
            .iter()
            .find(|v| &v.ident == *variant_name)
            .expect("Variant not found");
        let initialization = variant
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
            impl #impl_generics From<#wraps_type> for #enum_name #ty_generics #where_clause {
                fn from(v: #wraps_type) -> Self {
                    Self::#variant_name(#(#initialization),*)
                }
            }
        }
    }
}

impl GenerateFrom<'_, RegularStruct<'_, FromTraitInfo>> for FromTraitInfo {
    fn generate_from(&self, _data: &RegularStruct<'_, FromTraitInfo>) -> proc_macro2::TokenStream {
        quote! {}
    }
}

impl GenerateFrom<'_, RegularEnum<'_, FromTraitInfo>> for FromTraitInfo {
    fn generate_from(&self, _data: &RegularEnum<'_, FromTraitInfo>) -> proc_macro2::TokenStream {
        quote! {}
    }
}

impl GenerateFrom<'_, RegularVariant<'_, FromTraitInfo>> for FromTraitInfo {
    fn generate_from(&self, _data: &RegularVariant<'_, FromTraitInfo>) -> proc_macro2::TokenStream {
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
