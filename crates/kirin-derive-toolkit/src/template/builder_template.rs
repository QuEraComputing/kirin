use crate::context::{DeriveContext, InputMeta};
use crate::generators::builder::helpers::{
    build_fn_for_statement, build_fn_name, build_result_impl, build_result_module,
    build_result_module_enum, enum_build_fn, from_impl,
};
use crate::generators::builder::statement::StatementInfo;
use crate::ir::StandardLayout;
use proc_macro2::TokenStream;
use quote::quote;

use super::Template;

/// Generates constructor functions (`new()` / `op_*()`) and result types for IR statements.
///
/// Produces:
/// - Per-statement builder functions that allocate SSA values
/// - A result module with typed return structs
/// - `From` impls for wrapper variants
pub struct BuilderTemplate {
    default_crate_path: syn::Path,
}

impl BuilderTemplate {
    pub fn new() -> Self {
        Self {
            default_crate_path: syn::parse_quote!(::kirin::ir),
        }
    }

    pub fn with_crate_path(crate_path: impl Into<String>) -> Self {
        Self {
            default_crate_path: syn::parse_str(&crate_path.into()).unwrap(),
        }
    }

    fn full_crate_path(&self, meta: &InputMeta) -> syn::Path {
        meta.path_builder(&self.default_crate_path).full_crate_path()
    }

    fn build_statement_info(
        &self,
        meta: &InputMeta,
        stmt: &crate::ir::Statement<StandardLayout>,
    ) -> StatementInfo {
        let fields = stmt.collect_fields();
        let fn_name = build_fn_name(meta.is_enum, stmt);
        StatementInfo {
            name: stmt.name.clone(),
            fields,
            build_fn_name: fn_name,
            is_wrapper: stmt.wraps.is_some(),
            wrapper_type: stmt.wraps.as_ref().map(|w| w.ty.clone()),
        }
    }

    fn emit_for_struct(
        &self,
        ctx: &DeriveContext<'_, StandardLayout>,
        data: &crate::ir::DataStruct<StandardLayout>,
    ) -> darling::Result<TokenStream> {
        let meta = &ctx.meta;
        if meta.builder.is_none() {
            return Ok(TokenStream::new());
        }

        let info = self.build_statement_info(meta, &data.0);
        if info.is_wrapper {
            return Ok(from_impl(meta, &info));
        }

        let crate_path = self.full_crate_path(meta);
        let build_result_mod = build_result_module(meta, &info, &data.0, &crate_path);
        let build_fn =
            build_fn_for_statement(&info, meta, &crate_path, false)
                .unwrap_or_else(|err| err.write_errors());

        Ok(quote! {
            #build_fn
            #build_result_mod
        })
    }

    fn emit_for_enum(
        &self,
        ctx: &DeriveContext<'_, StandardLayout>,
        data: &crate::ir::DataEnum<StandardLayout>,
    ) -> darling::Result<TokenStream> {
        let meta = &ctx.meta;
        if meta.builder.is_none() {
            return Ok(TokenStream::new());
        }

        let crate_path = self.full_crate_path(meta);
        let all_wrappers = data.variants.iter().all(|v| v.wraps.is_some());

        let build_fn = if all_wrappers {
            TokenStream::new()
        } else {
            enum_build_fn(meta, data, |statement| {
                let info = self.build_statement_info(meta, statement);
                build_fn_for_statement(&info, meta, &crate_path, true)
            })?
        };

        let build_result_mod = if all_wrappers {
            TokenStream::new()
        } else {
            build_result_module_enum(meta, data, &crate_path, |statement| {
                let info = self.build_statement_info(meta, statement);
                build_result_impl(&info, statement)
            })?
        };

        let from_impls: Vec<_> = data
            .variants
            .iter()
            .filter(|stmt| stmt.wraps.is_some())
            .map(|stmt| {
                let info = self.build_statement_info(meta, stmt);
                from_impl(meta, &info)
            })
            .collect();

        Ok(quote! {
            #build_fn
            #build_result_mod
            #(#from_impls)*
        })
    }
}

impl Default for BuilderTemplate {
    fn default() -> Self {
        Self::new()
    }
}

impl Template<StandardLayout> for BuilderTemplate {
    fn emit(&self, ctx: &DeriveContext<'_, StandardLayout>) -> darling::Result<Vec<TokenStream>> {
        let tokens = match &ctx.input.data {
            crate::ir::Data::Struct(data) => self.emit_for_struct(ctx, data)?,
            crate::ir::Data::Enum(data) => self.emit_for_enum(ctx, data)?,
        };

        if tokens.is_empty() {
            Ok(vec![])
        } else {
            Ok(vec![tokens])
        }
    }
}
