use crate::context::{DeriveContext, StatementContext};
use crate::ir::{self, StandardLayout};
use crate::tokens::DelegationCall;
use proc_macro2::TokenStream;
use quote::{ToTokens, quote};

use super::MethodPattern;

/// Reads a boolean property value from IR attributes.
pub trait PropertyValueReader {
    fn global_value(&self, input: &ir::Input<StandardLayout>) -> bool;
    fn statement_value(&self, statement: &ir::Statement<StandardLayout>) -> bool;
    fn validate(&self, _input: &ir::Input<StandardLayout>) -> darling::Result<()> {
        Ok(())
    }
}

/// Built-in property kinds matching `#[kirin(...)]` attributes.
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

/// Reads a bare attribute like `#[callable]` from raw attrs.
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

/// Method pattern that returns a boolean based on `#[kirin(...)]` attributes or bare attrs.
///
/// For wrapper variants, delegates to the wrapped type's trait method.
/// For non-wrapper variants, returns `global_value || statement_value`.
pub struct BoolProperty {
    reader: Box<dyn PropertyValueReader>,
    trait_path: syn::Path,
    trait_method: syn::Ident,
    default_crate_path: syn::Path,
}

impl BoolProperty {
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
