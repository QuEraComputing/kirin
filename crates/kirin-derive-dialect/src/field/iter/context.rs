use kirin_derive_core::misc::{from_str, to_camel_case};
use kirin_derive_core::prelude::*;
use kirin_derive_core::tokens::FieldPatternTokens;
use quote::format_ident;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug)]
pub enum FieldIterKind {
    Arguments,
    Results,
    Blocks,
    Successors,
    Regions,
}

pub struct DeriveFieldIter {
    pub field_kind: FieldIterKind,
    pub default_crate_path: syn::Path,
    pub trait_path: syn::Path,
    pub trait_lifetime: syn::Lifetime,
    pub trait_method: syn::Ident,
    pub trait_type_iter: syn::Ident,
    pub matching_type: syn::Path,
    pub iter_name: syn::Ident,
    pub mutable: bool,
    pub(crate) input: Option<InputMeta>,
    pub(crate) statements: HashMap<String, StatementInfo>,
}

#[derive(Clone, Debug)]
pub(crate) struct StatementInfo {
    pub(crate) name: syn::Ident,
    pub(crate) pattern: FieldPatternTokens,
    pub(crate) pattern_empty: bool,
    pub(crate) iter_expr: proc_macro2::TokenStream,
    pub(crate) inner_type: proc_macro2::TokenStream,
    pub(crate) is_wrapper: bool,
}

impl DeriveFieldIter {
    pub fn new(
        field_kind: FieldIterKind,
        mutable: bool,
        default_crate_path: impl Into<String>,
        trait_path: impl Into<String>,
        matching_type: impl Into<String>,
        trait_method: impl Into<String>,
        trait_type_iter: impl Into<String>,
    ) -> Self {
        let trait_method: syn::Ident = from_str(trait_method);
        let iter_name = format_ident!("{}Iter", to_camel_case(trait_method.to_string()));
        Self {
            field_kind,
            default_crate_path: from_str(default_crate_path),
            trait_path: from_str(trait_path),
            trait_lifetime: from_str("'a"),
            trait_method,
            trait_type_iter: from_str(trait_type_iter),
            matching_type: from_str(matching_type),
            iter_name,
            mutable,
            input: None,
            statements: HashMap::new(),
        }
    }

    pub fn with_trait_lifetime(mut self, lifetime: impl Into<String>) -> Self {
        self.trait_lifetime = from_str(lifetime);
        self
    }

    pub fn with_iter_name(mut self, iter_name: impl Into<String>) -> Self {
        self.iter_name = from_str(iter_name);
        self
    }

    pub fn emit(&mut self, input: &syn::DeriveInput) -> darling::Result<proc_macro2::TokenStream> {
        let input = ir::Input::<StandardLayout>::from_derive_input(input)?;
        self.scan_input(&input)?;
        self.emit_input(&input)
    }

    pub(crate) fn input_ctx(&self) -> darling::Result<&InputMeta> {
        self.input.as_ref().ok_or_else(|| {
            darling::Error::custom("DeriveFieldIter context missing, call scan_input first")
        })
    }

    pub(crate) fn statement_info(
        &self,
        statement: &ir::Statement<StandardLayout>,
    ) -> darling::Result<&StatementInfo> {
        let key = statement.name.to_string();
        self.statements.get(&key).ok_or_else(|| {
            darling::Error::custom(format!(
                "Missing statement info for '{}', call scan_statement first",
                key
            ))
        })
    }
}
