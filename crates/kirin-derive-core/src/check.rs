use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};

use crate::data::*;

#[macro_export]
macro_rules! derive_check {
    ($input:expr, $method_name:ident, $trait_path:expr) => {
        // let name = stringify!($method_name);
        {
            let method_name_str = stringify!($method_name);
            let ctx = Context::new(
                CheckTraitInfo::new(method_name_str, syn::parse_quote! { $trait_path }, |attr| {
                    attr.$method_name.unwrap_or(false)
                }),
                $input,
            );
            let data = DataTrait::new(&ctx);
            ctx.generate_from(&data)
        }
    };
}

pub struct CheckTraitInfo {
    f: fn(&KirinAttribute) -> bool,
    method_name: syn::Ident,
    trait_path: syn::Path,
    generics: syn::Generics,
}

impl CheckTraitInfo {
    pub fn new(
        method_name: impl AsRef<str>,
        trait_path: syn::Path,
        f: fn(&KirinAttribute) -> bool,
    ) -> Self {
        Self {
            method_name: format_ident!("{}", method_name.as_ref()),
            trait_path,
            generics: syn::Generics::default(),
            f,
        }
    }
}

impl<'input> TraitInfo<'input> for CheckTraitInfo {
    type GlobalAttributeData = bool;
    type MatchingFields = bool;
    fn method_name(&self) -> &syn::Ident {
        &self.method_name
    }

    fn trait_generics(&self) -> &syn::Generics {
        &self.generics
    }

    fn trait_path(&self) -> &syn::Path {
        &self.trait_path
    }
}

impl FromStructFields<'_, CheckTraitInfo> for bool {
    fn from_struct_fields(
        ctx: &Context<'_, CheckTraitInfo>,
        _parent: &'_ syn::DataStruct,
        _fields: &'_ syn::Fields,
    ) -> Self {
        ctx.data || (ctx.trait_info.f)(&ctx.kirin_attr)
    }
}

impl FromVariantFields<'_, CheckTraitInfo> for bool {
    fn from_variant_fields(
        ctx: &Context<'_, CheckTraitInfo>,
        parent: &'_ syn::Variant,
        _fields: &'_ syn::Fields,
    ) -> Self {
        ctx.data || {
            let attr = KirinAttribute::from_attrs(&parent.attrs);
            (ctx.trait_info.f)(&attr)
        }
    }
}

impl GenerateFrom<'_, NamedWrapperStruct<'_, CheckTraitInfo>> for CheckTraitInfo {
    fn generate_from(
        &self,
        data: &NamedWrapperStruct<'_, CheckTraitInfo>,
    ) -> proc_macro2::TokenStream {
        let name = &data.ctx.input.ident;
        let method_name = &self.method_name;
        let trait_path = &self.trait_path;
        let wraps = &data.wraps;
        let wraps_type = &data.wraps_type;
        let (impl_generics, trait_ty_generics, input_type_generics, where_clause) =
            data.ctx.split_for_impl();

        quote! {
            impl #impl_generics #trait_path #trait_ty_generics for #name #input_type_generics #where_clause {
                fn #method_name(&self) -> bool {
                    let Self { #wraps, .. } = self;
                    <#wraps_type as #trait_path>::#method_name(#wraps)
                }
            }
        }
    }
}

impl GenerateFrom<'_, UnnamedWrapperStruct<'_, CheckTraitInfo>> for CheckTraitInfo {
    fn generate_from(
        &self,
        data: &UnnamedWrapperStruct<'_, CheckTraitInfo>,
    ) -> proc_macro2::TokenStream {
        let name = &data.ctx.input.ident;
        let method_name = &self.method_name;
        let trait_path = &self.trait_path;
        let wraps_index = data.wraps;
        let wraps_type = &data.wraps_type;
        let (impl_generics, trait_ty_generics, input_type_generics, where_clause) =
            data.ctx.split_for_impl();
        let vars = (0..=wraps_index)
            .map(|i| syn::Ident::new(&format!("field_{}", i), Span::call_site()))
            .collect::<Vec<_>>();
        let wraps_name = &vars[wraps_index];

        quote! {
            impl #impl_generics #trait_path #trait_ty_generics for #name #input_type_generics #where_clause {
                fn #method_name(&self) -> bool {
                    let Self (#(#vars,)* ..) = self;
                    <#wraps_type as #trait_path>::#method_name(#wraps_name)
                }
            }
        }
    }
}

impl GenerateFrom<'_, RegularStruct<'_, CheckTraitInfo>> for CheckTraitInfo {
    fn generate_from(&self, data: &RegularStruct<'_, CheckTraitInfo>) -> TokenStream {
        let name = &data.ctx.input.ident;
        let method_name = &self.method_name;
        let trait_path = &self.trait_path;
        let (impl_generics, trait_ty_generics, input_type_generics, where_clause) =
            data.ctx.split_for_impl();
        let value = data.fields;

        quote::quote! {
            impl #impl_generics #trait_path #trait_ty_generics for #name #input_type_generics #where_clause {
                fn #method_name(&self) -> bool {
                    #value
                }
            }
        }
    }
}

impl GenerateFrom<'_, RegularEnum<'_, CheckTraitInfo>> for CheckTraitInfo {
    fn generate_from(&self, data: &RegularEnum<'_, CheckTraitInfo>) -> TokenStream {
        let name = &data.ctx.input.ident;
        let method_name = &self.method_name;
        let trait_path = &self.trait_path;
        let (impl_generics, trait_ty_generics, input_type_generics, where_clause) =
            data.ctx.split_for_impl();

        let method_arms = data.variants.iter().map(|variant| variant.method_arm());

        quote::quote! {
            impl #impl_generics #trait_path #trait_ty_generics for #name #input_type_generics #where_clause {
                fn #method_name(&self) -> bool {
                    match self {
                        #(#method_arms)*
                    }
                }
            }
        }
    }
}

impl GenerateFrom<'_, WrapperEnum<'_, CheckTraitInfo>> for CheckTraitInfo {
    fn generate_from(&self, data: &WrapperEnum<'_, CheckTraitInfo>) -> TokenStream {
        let name = &data.ctx.input.ident;
        let method_name = &self.method_name;
        let trait_path = &self.trait_path;

        let (trait_impl_generics, trait_ty_generics, input_type_generics, trait_where_clause) =
            data.ctx.split_for_impl();
        let method_arms = data.variants.iter().map(|variant| variant.method_arm());

        quote! {
            impl #trait_impl_generics #trait_path #trait_ty_generics for #name #input_type_generics #trait_where_clause {
                fn #method_name(&self) -> bool {
                    match self {
                        #(#method_arms)*
                    }
                }
            }
        }
    }
}

impl GenerateFrom<'_, EitherEnum<'_, CheckTraitInfo>> for CheckTraitInfo {
    fn generate_from(&self, data: &EitherEnum<'_, CheckTraitInfo>) -> TokenStream {
        let name = &data.ctx.input.ident;
        let method_name = &self.method_name;
        let trait_path = &self.trait_path;

        let (trait_impl_generics, trait_ty_generics, input_type_generics, trait_where_clause) =
            data.ctx.split_for_impl();
        let method_arms = data.variants.iter().map(|variant| variant.method_arm());

        quote! {
            impl #trait_impl_generics #trait_path #trait_ty_generics for #name #input_type_generics #trait_where_clause {
                fn #method_name(&self) -> bool {
                    match self {
                        #(#method_arms)*
                    }
                }
            }
        }
    }
}

pub trait MethodArm {
    fn method_arm(&self) -> TokenStream;
}

impl MethodArm for WrapperVariant<'_, CheckTraitInfo> {
    fn method_arm(&self) -> TokenStream {
        match self {
            WrapperVariant::Named(v) => v.method_arm(),
            WrapperVariant::Unnamed(v) => v.method_arm(),
        }
    }
}

impl MethodArm for WrapperOrRegularVariant<'_, CheckTraitInfo> {
    fn method_arm(&self) -> TokenStream {
        match self {
            WrapperOrRegularVariant::Wrapper(v) => v.method_arm(),
            WrapperOrRegularVariant::Regular(v) => v.method_arm(),
        }
    }
}

impl MethodArm for NamedWrapperVariant<'_, CheckTraitInfo> {
    fn method_arm(&self) -> TokenStream {
        let wraps = &self.wraps;
        let wraps_type = &self.wraps_type;
        let variant_name = &self.variant_name;
        let trait_path = &self.ctx.trait_info.trait_path;
        let method_name = &self.ctx.trait_info.method_name;

        quote! {
            Self::#variant_name { #wraps, .. } => {
                <#wraps_type as #trait_path>::#method_name(#wraps)
            },
        }
    }
}

impl MethodArm for UnnamedWrapperVariant<'_, CheckTraitInfo> {
    fn method_arm(&self) -> TokenStream {
        let wraps_type = &self.wraps_type;
        let variant_name = &self.variant_name;
        let trait_path = &self.ctx.trait_info.trait_path;
        let method_name = &self.ctx.trait_info.method_name;
        let vars = (0..=self.wraps)
            .map(|i| format_ident!("field_{}", i))
            .collect::<Vec<_>>();
        let wraps = &vars[self.wraps];

        quote! {
            Self::#variant_name(#(#vars,)* ..) => {
                <#wraps_type as #trait_path>::#method_name(#wraps)
            },
        }
    }
}

impl MethodArm for RegularVariant<'_, CheckTraitInfo> {
    fn method_arm(&self) -> TokenStream {
        let variant_name = &self.variant_name;
        let matching_fields = &self.matching_fields;

        quote! {
            Self::#variant_name { .. } => #matching_fields,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::tests::rustfmt;

    use super::*;

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
                Variant3(T, #[kirin(wraps, constant)] Other),
            }
        };
        insta::assert_snapshot!(generate(input));
    }

    fn generate(input: syn::DeriveInput) -> String {
        let trait_info = CheckTraitInfo::new(
            "is_constant",
            syn::parse_quote! { ::kirin_ir::CheckConstant },
            |attr: &KirinAttribute| attr.is_constant.unwrap_or(false),
        );

        let ctx = Context::new(trait_info, &input);
        let data = DataTrait::new(&ctx);
        rustfmt(ctx.generate_from(&data))
    }
}
