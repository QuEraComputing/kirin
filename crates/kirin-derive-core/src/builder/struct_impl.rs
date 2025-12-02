use super::data::Builder;
use crate::{data::*, utils::to_camel_case};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};

impl<'a> GenerateFrom<'a, RegularStruct<'a, Builder>> for Builder {
    fn generate_from(&self, data: &RegularStruct<'a, Builder>) -> TokenStream {
        if !data.attrs.builder.is_enabled() {
            return quote! {};
        }

        let syn::Data::Struct(data_struct) = &data.input.data else {
            return syn::Error::new_spanned(
                &data.input.ident,
                "RegularStruct can only be created from struct data",
            )
            .to_compile_error();
        };

        let name = &data.input.ident;
        let crate_path = data.crate_root_path(self);
        let statement = format_ident!("{}_statement", name.to_string().to_lowercase());
        let statement_id = format_ident!("{}_statement_id", name.to_string().to_lowercase());
        let type_lattice = data.attrs.ty_lattice.clone();

        let builder_name = data.attrs.builder.builder_name(&format_ident!("new"));
        let ref_struct_name =
            format_ident!("{}{}Ref", name, to_camel_case(builder_name.to_string()));

        let inputs = data.fields.inputs(&crate_path);
        let results = data.fields.build_results(&crate_path, &statement_id);
        let others = data.fields.build_inputs(&crate_path);
        let result_names = data.fields.result_names();
        let ref_struct = data.fields.ref_struct(&crate_path, &ref_struct_name);
        let initialization = data.fields.initialization(&data_struct.fields);

        let (impl_generics, ty_generics, where_clause) = data.input.generics.split_for_impl();

        let header = if results.is_empty() {
            quote! {
                pub fn #builder_name<Lang: Language + From<#name #ty_generics>> (
                    context: &mut #crate_path::Context<Lang>,
                    #(#inputs,)*
                ) -> #ref_struct_name
            }
        } else if type_lattice.is_none() {
            return syn::Error::new_spanned(
                &data.input.ident,
                "missing #[kirin(type_lattice = ...)], cannot generate the builder",
            )
            .to_compile_error();
        } else {
            let type_lattice = type_lattice.unwrap();
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
            impl #impl_generics #name #ty_generics #where_clause {
                #header
                {
                    let #statement_id = context.new_statement_id();
                    #(#others)*

                    #(#results)*
                    let #statement = context
                        .statement()
                        .definition(Self #initialization)
                        .new();

                    #ref_struct_name {
                        id: #statement_id,
                        #(#result_names),*
                    }
                }
            }
            #ref_struct
        }
    }
}

impl<'a> GenerateFrom<'a, Struct<'a, Builder>> for Builder {
    fn generate_from(&self, data: &Struct<'a, Builder>) -> TokenStream {
        match data {
            Struct::Regular(data) => self.generate_from(data),
            Struct::Wrapper(_data) => quote! {},
        }
    }
}
