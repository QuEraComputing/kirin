mod generate;
mod helpers;

pub use generate::BuilderTemplate;

use crate::context::DeriveContext;
use crate::ir::{self, StandardLayout};
use proc_macro2::TokenStream;

use super::Template;

impl Template<StandardLayout> for BuilderTemplate {
    fn emit(&self, ctx: &DeriveContext<'_, StandardLayout>) -> darling::Result<Vec<TokenStream>> {
        let tokens = match &ctx.input.data {
            ir::Data::Struct(data) => self.emit_for_struct(ctx, data)?,
            ir::Data::Enum(data) => self.emit_for_enum(ctx, data)?,
        };

        if tokens.is_empty() {
            Ok(vec![])
        } else {
            Ok(vec![tokens])
        }
    }
}
