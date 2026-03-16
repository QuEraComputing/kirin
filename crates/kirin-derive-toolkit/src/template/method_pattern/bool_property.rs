use crate::context::{DeriveContext, StatementContext};
use crate::ir::{self, StandardLayout};
use crate::tokens::DelegationCall;
use proc_macro2::TokenStream;
use quote::{ToTokens, quote};

use super::MethodPattern;

/// Reads a boolean property value from IR derive attributes.
///
/// Implementations extract a single boolean flag from either the top-level
/// `#[kirin(...)]` attributes or per-variant/per-field attributes. The two
/// levels are combined with logical OR in the generated code.
pub trait PropertyValueReader {
    /// Read the property from the top-level (struct/enum) attributes.
    fn global_value(&self, input: &ir::Input<StandardLayout>) -> bool;
    /// Read the property from a single statement (variant) attributes.
    fn statement_value(&self, statement: &ir::Statement<StandardLayout>) -> bool;
    /// Validate cross-attribute invariants (e.g., `constant` requires `pure`).
    fn validate(&self, _input: &ir::Input<StandardLayout>) -> darling::Result<()> {
        Ok(())
    }
}

/// Built-in property kinds corresponding to `#[kirin(...)]` boolean flags.
///
/// Each variant maps to the matching field on the parsed `KirinAttrs` struct
/// (e.g., `PropertyKind::Pure` reads `attrs.pure`).
#[derive(Clone, Copy, Debug)]
pub enum PropertyKind {
    /// `#[kirin(constant)]` -- requires `pure` to also be set.
    Constant,
    /// `#[kirin(pure)]`
    Pure,
    /// `#[kirin(speculatable)]` -- requires `pure` to also be set.
    Speculatable,
    /// `#[kirin(terminator)]`
    Terminator,
    /// `#[kirin(edge)]`
    Edge,
}

impl PropertyValueReader for PropertyKind {
    fn global_value(&self, input: &ir::Input<StandardLayout>) -> bool {
        match self {
            PropertyKind::Constant => input.attrs.constant,
            PropertyKind::Pure => input.attrs.pure,
            PropertyKind::Speculatable => input.attrs.speculatable,
            PropertyKind::Terminator => input.attrs.terminator,
            PropertyKind::Edge => input.attrs.edge,
        }
    }

    fn statement_value(&self, statement: &ir::Statement<StandardLayout>) -> bool {
        match self {
            PropertyKind::Constant => statement.attrs.constant,
            PropertyKind::Pure => statement.attrs.pure,
            PropertyKind::Speculatable => statement.attrs.speculatable,
            PropertyKind::Terminator => statement.attrs.terminator,
            PropertyKind::Edge => statement.attrs.edge,
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

/// Reads a bare attribute (e.g., `#[callable]`) from raw `syn::Attribute` lists.
///
/// Unlike [`PropertyKind`] which reads parsed `#[kirin(...)]` fields, this reader
/// checks for standalone helper attributes that are not namespaced under `#[kirin(...)]`.
pub struct BareAttrReader {
    /// The attribute identifier to look for (e.g., `"callable"`).
    attr_name: &'static str,
}

impl BareAttrReader {
    /// Create a reader that matches attributes with the given name.
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

/// Method pattern that returns a boolean based on `#[kirin(...)]` attributes or bare attrs.
///
/// For wrapper (`#[wraps]`) variants, delegates to the wrapped type's trait method
/// via fully-qualified syntax. For non-wrapper variants, emits a literal
/// `global_value || statement_value` expression.
pub struct BoolProperty {
    /// Strategy for reading the boolean flag from attributes.
    reader: Box<dyn PropertyValueReader>,
    /// Trait to delegate through for wrapper variants (e.g., `IsConstant`).
    trait_path: syn::Path,
    /// Method on the trait to call (e.g., `is_constant`).
    trait_method: syn::Ident,
    /// Default crate path prefix (e.g., `::kirin::ir`), overridden by `#[kirin(crate = ...)]`.
    default_crate_path: syn::Path,
}

impl BoolProperty {
    /// Create a new boolean property pattern.
    ///
    /// `reader` extracts the flag value, `trait_path` and `trait_method` are used
    /// for delegation on wrapper variants, and `default_crate_path` provides the
    /// fallback crate root when `#[kirin(crate = ...)]` is not specified.
    pub fn new(
        reader: impl PropertyValueReader + 'static,
        trait_path: syn::Path,
        trait_method: syn::Ident,
        default_crate_path: syn::Path,
    ) -> Self {
        Self {
            reader: Box::new(reader),
            trait_path,
            trait_method,
            default_crate_path,
        }
    }

    fn full_trait_path(&self, ctx: &DeriveContext<'_, StandardLayout>) -> syn::Path {
        ctx.meta
            .path_builder(&self.default_crate_path)
            .full_trait_path(&self.trait_path)
    }

    fn value_expr(
        &self,
        ctx: &DeriveContext<'_, StandardLayout>,
        stmt_ctx: &StatementContext<'_, StandardLayout>,
    ) -> TokenStream {
        if let (Some(wrapper_ty), Some(wrapper_field)) =
            (stmt_ctx.wrapper_type, &stmt_ctx.wrapper_binding)
        {
            let trait_path = self.full_trait_path(ctx);
            return DelegationCall {
                wrapper_ty: quote! { #wrapper_ty },
                trait_path: quote! { #trait_path },
                trait_method: self.trait_method.clone(),
                field: wrapper_field.clone(),
            }
            .to_token_stream();
        }

        let global = self.reader.global_value(ctx.input);
        if ctx.meta.is_enum {
            let stmt = self.reader.statement_value(stmt_ctx.stmt);
            quote! { #global || #stmt }
        } else {
            quote! { #global }
        }
    }
}

impl MethodPattern<StandardLayout> for BoolProperty {
    fn for_struct(
        &self,
        ctx: &DeriveContext<'_, StandardLayout>,
        stmt_ctx: &StatementContext<'_, StandardLayout>,
    ) -> darling::Result<TokenStream> {
        self.reader.validate(ctx.input)?;
        let value_expr = self.value_expr(ctx, stmt_ctx);
        if stmt_ctx.is_wrapper {
            let pattern = &stmt_ctx.pattern;
            Ok(quote! {
                let Self #pattern = self;
                #value_expr
            })
        } else {
            Ok(value_expr)
        }
    }

    fn for_variant(
        &self,
        ctx: &DeriveContext<'_, StandardLayout>,
        stmt_ctx: &StatementContext<'_, StandardLayout>,
    ) -> darling::Result<TokenStream> {
        self.reader.validate(ctx.input)?;
        Ok(self.value_expr(ctx, stmt_ctx))
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
                    darling::Error::custom("effective #[kirin(constant)] requires #[kirin(pure)]")
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
                let effective_speculatable = global_speculatable || statement.attrs.speculatable;
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
