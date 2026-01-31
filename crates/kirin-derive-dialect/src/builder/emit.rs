use crate::builder::context::DeriveBuilder;
use crate::builder::helpers::{
    build_fn_for_statement, build_result_impl, build_result_module, build_result_module_enum,
    enum_build_fn, from_impl, struct_build_fn,
};
use kirin_derive_core::prelude::*;
use quote::quote;

impl<'ir> Emit<'ir, StandardLayout> for DeriveBuilder {
    fn emit_struct(
        &mut self,
        data: &'ir ir::DataStruct<StandardLayout>,
    ) -> darling::Result<proc_macro2::TokenStream> {
        let input = self.input_ctx()?;
        if input.builder.is_none() {
            return Ok(proc_macro2::TokenStream::new());
        }

        let info = self.statement_info(&data.0)?;
        if info.is_wrapper {
            return Ok(from_impl(input, info));
        }

        let crate_path = self.full_crate_path(input);
        let build_result_mod = build_result_module(input, info, &data.0, &crate_path);
        let build_fn = struct_build_fn(input, info, &crate_path);

        Ok(quote! {
            #build_fn
            #build_result_mod
        })
    }

    fn emit_enum(
        &mut self,
        data: &'ir ir::DataEnum<StandardLayout>,
    ) -> darling::Result<proc_macro2::TokenStream> {
        let input = self.input_ctx()?;
        if input.builder.is_none() {
            return Ok(proc_macro2::TokenStream::new());
        }

        let crate_path = self.full_crate_path(input);
        let all_wrappers = data.variants.iter().all(|v| v.wraps.is_some());

        let build_fn = if all_wrappers {
            proc_macro2::TokenStream::new()
        } else {
            enum_build_fn(input, data, |statement| {
                let info = self.statement_info(statement)?;
                build_fn_for_statement(info, input, &crate_path, true)
            })?
        };

        let build_result_mod = if all_wrappers {
            proc_macro2::TokenStream::new()
        } else {
            build_result_module_enum(input, data, &crate_path, |statement| {
                let info = self.statement_info(statement)?;
                build_result_impl(info, statement)
            })?
        };

        let from_impls: Vec<_> = data
            .variants
            .iter()
            .filter_map(|statement| {
                if statement.wraps.is_some() {
                    let info = self.statement_info(statement).ok()?;
                    Some(from_impl(input, info))
                } else {
                    None
                }
            })
            .collect();

        Ok(quote! {
            #build_fn
            #build_result_mod
            #(#from_impls)*
        })
    }
}
