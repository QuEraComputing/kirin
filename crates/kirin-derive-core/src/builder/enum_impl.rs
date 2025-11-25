use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use super::data::Builder;
use crate::{
    data::{EitherEnum, EitherVariant, Enum, GenerateFrom, RegularEnum, RegularVariant},
    utils::{to_camel_case, to_snake_case},
};

impl<'a> GenerateFrom<'a, RegularEnum<'a, Builder>> for Builder {
    fn generate_from(&self, data: &RegularEnum<'a, Builder>) -> TokenStream {
        let name = &data.input.ident;
        let snake_case_name = to_snake_case(name.to_string());
        let statement = format_ident!("{}_statement", snake_case_name);
        let statement_id = format_ident!("{}_statement_id", snake_case_name);

        let (impl_generics, ty_generics, where_clause) = data.input.generics.split_for_impl();

        let type_lattice = data
            .attrs
            .ty_lattice
            .clone()
            .expect("missing #[kirin(type_lattice = ...)], cannot generate the builder");

        let variants = data.variants.iter().map(|variant| {
            variant_builder(
                name,
                &statement,
                &statement_id,
                &type_lattice,
                &data.input.generics,
                variant,
            )
        });

        let ref_structs = data
            .variants
            .iter()
            .map(|variant| ref_struct_builder(variant, name));

        quote! {
            impl #impl_generics #name #ty_generics #where_clause {
                #(#variants)*
            }
            #(#ref_structs)*
        }
    }
}

impl<'a> GenerateFrom<'a, EitherEnum<'a, Builder>> for Builder {
    fn generate_from(&self, data: &EitherEnum<'a, Builder>) -> TokenStream {
        let name = &data.input.ident;
        let snake_case_name = to_snake_case(name.to_string());
        let statement = format_ident!("{}_statement", snake_case_name);
        let statement_id = format_ident!("{}_statement_id", snake_case_name);

        let (impl_generics, ty_generics, where_clause) = data.input.generics.split_for_impl();

        let type_lattice = data
            .attrs
            .ty_lattice
            .clone()
            .expect("missing #[kirin(type_lattice = ...)], cannot generate the builder");

        let regular_variants = data
            .variants
            .iter()
            .filter_map(|variant| match variant {
                EitherVariant::Regular(v) => Some(v),
                EitherVariant::Wrapper(_) => None,
            })
            .collect::<Vec<_>>();

        let variant_builders = regular_variants.iter().map(|variant| {
            variant_builder(
                name,
                &statement,
                &statement_id,
                &type_lattice,
                &data.input.generics,
                variant,
            )
        });

        let ref_structs = regular_variants
            .iter()
            .map(|variant| ref_struct_builder(variant, name));

        quote! {
            impl #impl_generics #name #ty_generics #where_clause {
                #(#variant_builders)*
            }
            #(#ref_structs)*
        }
    }
}

fn variant_builder(
    name: &syn::Ident,
    statement: &syn::Ident,
    statement_id: &syn::Ident,
    type_lattice: &syn::Type,
    generics: &syn::Generics,
    variant: &RegularVariant<'_, Builder>,
) -> TokenStream {
    let (_, ty_generics, _) = generics.split_for_impl();
    let variant_name = variant.variant_name;
    let snake_case_variant_name = to_snake_case(variant_name.to_string());
    let builder_name = variant
        .attrs
        .builder
        .builder_name(&format_ident!("op_{}", snake_case_variant_name));
    let ref_struct_name = format_ident!("{}{}Ref", name, to_camel_case(builder_name.to_string()));

    let inputs = variant.fields.inputs();
    let results = variant.fields.build_results(&statement_id);
    let others = variant.fields.build_inputs();
    let result_names = variant.fields.result_names();
    let initialization = variant.fields.initialization(&variant.variant.fields);

    quote! {
        pub fn #builder_name<Lang: Language + From<#name #ty_generics>> (
            arena: &mut Arena<Lang>,
            #(#inputs,)*
        ) -> #ref_struct_name
        where
            Lang::TypeLattice: From<#type_lattice>,
        {
            let #statement_id = arena.new_statement_id();
            #(#others)*
            #(#results)*
            let #statement = arena
                .statement()
                .definition(Self #initialization)
                .new();

            #ref_struct_name {
                id: #statement_id,
                #(#result_names),*
            }
        }
    }
}

fn ref_struct_builder(variant: &RegularVariant<'_, Builder>, name: &syn::Ident) -> TokenStream {
    let variant_name = variant.variant_name;
    let snake_case_variant_name = to_snake_case(variant_name.to_string());
    let builder_name = variant
        .attrs
        .builder
        .builder_name(&format_ident!("op_{}", snake_case_variant_name));
    let ref_struct_name = format_ident!("{}{}Ref", name, to_camel_case(builder_name.to_string()));
    variant.fields.ref_struct(&ref_struct_name)
}

impl<'a> GenerateFrom<'a, Enum<'a, Builder>> for Builder {
    fn generate_from(&self, data: &Enum<'a, Builder>) -> TokenStream {
        match data {
            Enum::Regular(data) => self.generate_from(data),
            Enum::Either(data) => self.generate_from(data),
            Enum::Wrapper(_data) => quote! {},
        }
    }
}
