use crate::context::DeriveContext;
use crate::emit::Emit;
use crate::generators::builder::DeriveBuilder;
use crate::ir::StandardLayout;
use crate::scan::Scan;
use proc_macro2::TokenStream;

use super::Template;

/// Template wrapper around the existing DeriveBuilder generator.
///
/// The builder pattern is complex enough (constructor functions, result modules,
/// From impls) that it continues to use the existing generator infrastructure
/// rather than the MethodPattern approach.
pub struct BuilderTemplate {
    builder: DeriveBuilder,
}

impl BuilderTemplate {
    pub fn new() -> Self {
        Self {
            builder: DeriveBuilder::default(),
        }
    }

    pub fn with_crate_path(crate_path: impl Into<String>) -> Self {
        Self {
            builder: DeriveBuilder::new(crate_path),
        }
    }
}

impl Default for BuilderTemplate {
    fn default() -> Self {
        Self::new()
    }
}

impl Template<StandardLayout> for BuilderTemplate {
    fn emit(&self, ctx: &DeriveContext<'_, StandardLayout>) -> darling::Result<Vec<TokenStream>> {
        let mut builder = self.builder.clone();
        builder.scan_input(ctx.input)?;
        let tokens = builder.emit_input(ctx.input)?;
        if tokens.is_empty() {
            Ok(vec![])
        } else {
            Ok(vec![tokens])
        }
    }
}
