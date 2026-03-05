use crate::context::{DeriveContext, StatementContext};
use crate::ir::StandardLayout;
use proc_macro2::TokenStream;

use super::MethodPattern;

/// Placeholder for builder pattern generation.
///
/// The builder pattern is complex enough that it continues to use
/// the existing `generators::builder` infrastructure directly as a
/// Template rather than a MethodPattern. This module exists for
/// API completeness.
pub struct BuilderPattern;

impl MethodPattern<StandardLayout> for BuilderPattern {
    fn for_struct(
        &self,
        _ctx: &DeriveContext<'_, StandardLayout>,
        _stmt_ctx: &StatementContext<'_, StandardLayout>,
    ) -> darling::Result<TokenStream> {
        Err(darling::Error::custom(
            "BuilderPattern should be used via BuilderTemplate, not as a MethodPattern",
        ))
    }

    fn for_variant(
        &self,
        _ctx: &DeriveContext<'_, StandardLayout>,
        _stmt_ctx: &StatementContext<'_, StandardLayout>,
    ) -> darling::Result<TokenStream> {
        Err(darling::Error::custom(
            "BuilderPattern should be used via BuilderTemplate, not as a MethodPattern",
        ))
    }
}
