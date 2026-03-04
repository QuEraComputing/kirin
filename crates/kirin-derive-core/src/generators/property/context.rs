use crate::derive::InputMeta as CoreInputMeta;
use crate::generators::common;
use crate::generators::property::statement::StatementInfo;
use crate::misc::from_str;
use crate::prelude::*;
use std::collections::HashMap;

/// Reads boolean property values from derive input attributes.
///
/// Built-in properties (constant, pure, speculatable, terminator) implement
/// this via [`PropertyKind`], reading from darling-parsed `#[kirin(...)]`
/// attributes. Downstream properties can implement this trait to read from
/// custom bare attributes (e.g., `#[quantum]`) using [`BareAttrReader`].
pub trait PropertyValueReader {
    /// Read the global (type-level) property value.
    fn global_value(&self, input: &ir::Input<StandardLayout>) -> bool;

    /// Read the per-statement (variant-level) property value.
    fn statement_value(&self, statement: &ir::Statement<StandardLayout>) -> bool;

    /// Optional cross-property validation. Default: no validation.
    fn validate(&self, _input: &ir::Input<StandardLayout>) -> darling::Result<()> {
        Ok(())
    }
}

#[derive(Clone, Copy, Debug)]
pub enum PropertyKind {
    Constant,
    Pure,
    Speculatable,
    Terminator,
}

impl PropertyValueReader for PropertyKind {
    fn global_value(&self, input: &ir::Input<StandardLayout>) -> bool {
        match self {
            PropertyKind::Constant => input.attrs.constant,
            PropertyKind::Pure => input.attrs.pure,
            PropertyKind::Speculatable => input.attrs.speculatable,
            PropertyKind::Terminator => input.attrs.terminator,
        }
    }

    fn statement_value(&self, statement: &ir::Statement<StandardLayout>) -> bool {
        match self {
            PropertyKind::Constant => statement.attrs.constant,
            PropertyKind::Pure => statement.attrs.pure,
            PropertyKind::Speculatable => statement.attrs.speculatable,
            PropertyKind::Terminator => statement.attrs.terminator,
        }
    }

    fn validate(&self, input: &ir::Input<StandardLayout>) -> darling::Result<()> {
        match self {
            PropertyKind::Constant => validate_constant_pure(input),
            PropertyKind::Speculatable => validate_speculatable_pure(input),
            _ => Ok(()),
        }
    }
}

/// Reads a bare attribute (e.g., `#[quantum]`) from struct/variant raw attributes.
///
/// This enables downstream crates to define custom boolean property derives
/// without modifying `kirin-derive-core`'s attribute schema. The attribute is
/// read from the raw `syn::Attribute` list stored on `Input` and `Statement`.
pub struct BareAttrReader {
    attr_name: &'static str,
}

impl BareAttrReader {
    pub const fn new(attr_name: &'static str) -> Self {
        Self { attr_name }
    }
}

impl PropertyValueReader for BareAttrReader {
    fn global_value(&self, input: &ir::Input<StandardLayout>) -> bool {
        input
            .raw_attrs
            .iter()
            .any(|a| a.path().is_ident(self.attr_name))
    }

    fn statement_value(&self, statement: &ir::Statement<StandardLayout>) -> bool {
        statement
            .raw_attrs
            .iter()
            .any(|a| a.path().is_ident(self.attr_name))
    }
}

pub struct DeriveProperty {
    pub reader: Box<dyn PropertyValueReader>,
    pub default_crate_path: syn::Path,
    pub trait_path: syn::Path,
    pub trait_method: syn::Ident,
    pub value_type: syn::Type,
    pub(crate) input: Option<InputContext>,
    pub(crate) statements: HashMap<String, StatementInfo>,
}

#[derive(Clone, Debug)]
pub(crate) struct InputContext {
    pub(crate) core: CoreInputMeta,
    pub(crate) global_value: bool,
}

impl DeriveProperty {
    pub fn new(
        reader: impl PropertyValueReader + 'static,
        default_crate_path: impl Into<String>,
        trait_path: impl Into<String>,
        trait_method: impl Into<String>,
        value_type: impl Into<String>,
    ) -> Self {
        Self {
            reader: Box::new(reader),
            default_crate_path: from_str(default_crate_path),
            trait_path: from_str(trait_path),
            trait_method: from_str(trait_method),
            value_type: from_str(value_type),
            input: None,
            statements: HashMap::new(),
        }
    }

    /// Create a property derive that reads from a bare attribute (e.g., `#[quantum]`).
    ///
    /// This is the simplest way to define a custom boolean property derive
    /// without modifying kirin-derive-core's attribute schema.
    pub fn bare_attr(
        attr_name: &'static str,
        default_crate_path: impl Into<String>,
        trait_path: impl Into<String>,
        trait_method: impl Into<String>,
        value_type: impl Into<String>,
    ) -> Self {
        Self::new(
            BareAttrReader::new(attr_name),
            default_crate_path,
            trait_path,
            trait_method,
            value_type,
        )
    }

    /// Create a property derive with a custom [`PropertyValueReader`].
    ///
    /// Use this when you need more control than [`bare_attr`](Self::bare_attr)
    /// provides (e.g., custom validation or reading from nested attributes).
    pub fn with_reader(
        reader: impl PropertyValueReader + 'static,
        default_crate_path: impl Into<String>,
        trait_path: impl Into<String>,
        trait_method: impl Into<String>,
        value_type: impl Into<String>,
    ) -> Self {
        Self::new(reader, default_crate_path, trait_path, trait_method, value_type)
    }

    pub fn emit(&mut self, input: &syn::DeriveInput) -> darling::Result<proc_macro2::TokenStream> {
        common::emit_from_derive_input(self, input)
    }

    pub fn emit_from_input(
        &mut self,
        input: &ir::Input<StandardLayout>,
    ) -> darling::Result<proc_macro2::TokenStream> {
        common::emit_from_ir(self, input)
    }

    pub(crate) fn input_ctx(&self) -> darling::Result<&InputContext> {
        common::require_input_ctx(&self.input, "DeriveProperty")
    }

    pub(crate) fn statement_info(
        &self,
        statement: &ir::Statement<StandardLayout>,
    ) -> darling::Result<&StatementInfo> {
        common::statement_info(&self.statements, statement)
    }

    pub(crate) fn full_trait_path(&self, input: &InputContext) -> syn::Path {
        input
            .core
            .path_builder(&self.default_crate_path)
            .full_trait_path(&self.trait_path)
    }
}

fn validate_constant_pure(input: &ir::Input<StandardLayout>) -> darling::Result<()> {
    let mut errors = darling::Error::accumulator();
    let global_constant = input.attrs.constant;
    let global_pure = input.attrs.pure;

    match &input.data {
        ir::Data::Struct(statement) => {
            if statement.wraps.is_none() && global_constant && !global_pure {
                errors.push(
                    darling::Error::custom(
                        "effective #[kirin(constant)] requires #[kirin(pure)]",
                    )
                    .with_span(&input.name),
                );
            }
        }
        ir::Data::Enum(data) => {
            for statement in data.iter() {
                if statement.wraps.is_some() {
                    continue;
                }
                let effective_constant = global_constant || statement.attrs.constant;
                let effective_pure = global_pure || statement.attrs.pure;
                if effective_constant && !effective_pure {
                    errors.push(
                        darling::Error::custom(format!(
                            "variant '{}' is effectively #[kirin(constant)] but not #[kirin(pure)]",
                            statement.name
                        ))
                        .with_span(&statement.name),
                    );
                }
            }
        }
    }

    errors.finish()
}

fn validate_speculatable_pure(input: &ir::Input<StandardLayout>) -> darling::Result<()> {
    let mut errors = darling::Error::accumulator();
    let global_speculatable = input.attrs.speculatable;
    let global_pure = input.attrs.pure;

    match &input.data {
        ir::Data::Struct(statement) => {
            if statement.wraps.is_none() && global_speculatable && !global_pure {
                errors.push(
                    darling::Error::custom(
                        "effective #[kirin(speculatable)] requires #[kirin(pure)]",
                    )
                    .with_span(&input.name),
                );
            }
        }
        ir::Data::Enum(data) => {
            for statement in data.iter() {
                if statement.wraps.is_some() {
                    continue;
                }
                let effective_speculatable =
                    global_speculatable || statement.attrs.speculatable;
                let effective_pure = global_pure || statement.attrs.pure;
                if effective_speculatable && !effective_pure {
                    errors.push(
                        darling::Error::custom(format!(
                            "variant '{}' is effectively #[kirin(speculatable)] but not #[kirin(pure)]",
                            statement.name
                        ))
                        .with_span(&statement.name),
                    );
                }
            }
        }
    }

    errors.finish()
}
