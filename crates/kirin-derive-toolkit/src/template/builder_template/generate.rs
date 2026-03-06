use super::helpers::{self, StatementInfo};
use crate::context::{DeriveContext, InputMeta};
use crate::ir::{self, StandardLayout};
use proc_macro2::TokenStream;
use quote::quote;

/// Generates constructor functions (`new()` / `op_*()`) and result types for IR statements.
///
/// Produces:
/// - Per-statement builder functions that allocate SSA values
/// - A result module with typed return structs
/// - `From` impls for wrapper variants
///
/// Activated by `#[kirin(fn)]` on the type. Skips wrapper variants (emits `From` impls instead).
pub struct BuilderTemplate {
    default_crate_path: syn::Path,
}

impl BuilderTemplate {
    /// Create a builder template using the default crate path (`::kirin::ir`).
    pub fn new() -> Self {
        Self {
            default_crate_path: syn::parse_quote!(::kirin::ir),
        }
    }

    /// Create a builder template with a custom crate path (e.g., `"kirin_ir"`).
    pub fn with_crate_path(crate_path: impl Into<String>) -> Self {
        Self {
            default_crate_path: syn::parse_str(&crate_path.into()).unwrap(),
        }
    }

    pub(super) fn full_crate_path(&self, meta: &InputMeta) -> syn::Path {
        meta.path_builder(&self.default_crate_path)
            .full_crate_path()
    }

    pub(super) fn build_statement_info(
        meta: &InputMeta,
        stmt: &ir::Statement<StandardLayout>,
    ) -> StatementInfo {
        let fields = stmt.collect_fields();
        let fn_name = helpers::build_fn_name(meta.is_enum, stmt);
        StatementInfo {
            name: stmt.name.clone(),
            fields,
            build_fn_name: fn_name,
            is_wrapper: stmt.wraps.is_some(),
            wrapper_type: stmt.wraps.as_ref().map(|w| w.ty.clone()),
        }
    }

    pub(crate) fn emit_for_struct(
        &self,
        ctx: &DeriveContext<'_, StandardLayout>,
        data: &ir::DataStruct<StandardLayout>,
    ) -> darling::Result<TokenStream> {
        let meta = &ctx.meta;
        if meta.builder.is_none() {
            return Ok(TokenStream::new());
        }

        let info = Self::build_statement_info(meta, &data.0);
        if info.is_wrapper {
            return Ok(helpers::from_impl(meta, &info));
        }

        let crate_path = self.full_crate_path(meta);
        let build_result_mod = helpers::build_result_module(meta, &info, &data.0, &crate_path);
        let build_fn = helpers::build_fn_for_statement(&info, meta, &crate_path, false)
            .unwrap_or_else(|err| err.write_errors());

        Ok(quote! {
            #build_fn
            #build_result_mod
        })
    }

    pub(crate) fn emit_for_enum(
        &self,
        ctx: &DeriveContext<'_, StandardLayout>,
        data: &ir::DataEnum<StandardLayout>,
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
            helpers::enum_build_fn(meta, data, |statement| {
                let info = Self::build_statement_info(meta, statement);
                helpers::build_fn_for_statement(&info, meta, &crate_path, true)
            })?
        };

        let build_result_mod = if all_wrappers {
            TokenStream::new()
        } else {
            helpers::build_result_module_enum(meta, data, &crate_path, |statement| {
                let info = Self::build_statement_info(meta, statement);
                helpers::build_result_impl(&info, statement)
            })?
        };

        let from_impls: Vec<_> = data
            .variants
            .iter()
            .filter(|stmt| stmt.wraps.is_some())
            .map(|stmt| {
                let info = Self::build_statement_info(meta, stmt);
                helpers::from_impl(meta, &info)
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
