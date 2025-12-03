use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use super::data::Builder;
use crate::{
    data::*,
    utils::{to_camel_case, to_snake_case},
};

impl<'a> GenerateFrom<'a, RegularEnum<'a, Builder>> for Builder {
    fn generate_from(&self, data: &RegularEnum<'a, Builder>) -> TokenStream {
        if !data.attrs.builder.is_enabled() {
            return quote! {};
        }

        let name = &data.input.ident;
        let crate_path = data.crate_root_path(self);
        let snake_case_name = to_snake_case(name.to_string());
        let statement = format_ident!("{}_statement", snake_case_name);
        let statement_id = format_ident!("{}_statement_id", snake_case_name);

        let (impl_generics, ty_generics, where_clause) = data.input.generics.split_for_impl();
        let variants = data.variants.iter().map(|variant| {
            variant_builder(
                &crate_path,
                name,
                &statement,
                &statement_id,
                &data.attrs.ty_lattice,
                &data.input.generics,
                variant,
            )
        });

        let ref_structs = data
            .variants
            .iter()
            .map(|variant| ref_struct_builder(&crate_path, variant, name));

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
        if !data.attrs.builder.is_enabled() {
            return quote! {};
        }

        let name = &data.input.ident;
        let crate_path = data.crate_root_path(self);
        let snake_case_name = to_snake_case(name.to_string());
        let statement = format_ident!("{}_statement", snake_case_name);
        let statement_id = format_ident!("{}_statement_id", snake_case_name);

        let (impl_generics, ty_generics, where_clause) = data.input.generics.split_for_impl();

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
                &crate_path,
                name,
                &statement,
                &statement_id,
                &data.attrs.ty_lattice,
                &data.input.generics,
                variant,
            )
        });

        let ref_structs = regular_variants
            .iter()
            .map(|variant| ref_struct_builder(&crate_path, variant, name));

        quote! {
            impl #impl_generics #name #ty_generics #where_clause {
                #(#variant_builders)*
            }
            #(#ref_structs)*
        }
    }
}

fn variant_builder(
    crate_path: &syn::Path,
    name: &syn::Ident,
    statement: &syn::Ident,
    statement_id: &syn::Ident,
    type_lattice: &Option<syn::Type>,
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

    let inputs = variant.fields.inputs(crate_path);
    let results = variant.fields.build_results(crate_path, &statement_id);
    let others = variant.fields.build_inputs(crate_path);
    let result_names = variant.fields.result_names();
    let initialization = variant.fields.initialization(&variant.variant.fields);

    let header = if results.is_empty() {
        quote! {
            pub fn #builder_name<Lang: Language + From<#name #ty_generics>> (
                context: &mut #crate_path::Context<Lang>,
                #(#inputs,)*
            ) -> #ref_struct_name
        }
    } else if type_lattice.is_none() {
        return syn::Error::new_spanned(
            &variant_name,
            "missing #[kirin(type_lattice = ...)], cannot generate the builder",
        )
        .to_compile_error();
    } else {
        let type_lattice = type_lattice.clone().unwrap();
        quote! {
            pub fn #builder_name<Lang: Language + From<#name #ty_generics>> (
                context: &mut #crate_path::Context<Lang>,
                #(#inputs,)*
            ) -> #ref_struct_name
            where
                Lang::TypeLattice: From<#type_lattice>,
        }
    };

    quote! {
        #header
        {
            let #statement_id = context.statement_arena().next_id();
            #(#others)*
            #(#results)*
            let #statement = context
                .statement()
                .definition(#name::#variant_name #initialization)
                .new();

            #ref_struct_name {
                id: #statement_id,
                #(#result_names),*
            }
        }
    }
}

fn ref_struct_builder(
    crate_path: &syn::Path,
    variant: &RegularVariant<'_, Builder>,
    name: &syn::Ident,
) -> TokenStream {
    let variant_name = variant.variant_name;
    let snake_case_variant_name = to_snake_case(variant_name.to_string());
    let builder_name = variant
        .attrs
        .builder
        .builder_name(&format_ident!("op_{}", snake_case_variant_name));
    let ref_struct_name = format_ident!("{}{}Ref", name, to_camel_case(builder_name.to_string()));
    variant.fields.ref_struct(crate_path, &ref_struct_name)
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
