mod input_meta;
mod statement;

pub use input_meta::{InputMeta, PathBuilder};
pub use statement::StatementContext;

use indexmap::IndexMap;
use proc_macro2::TokenStream;
use quote::ToTokens;

use crate::ir::{Input, Layout};
use statement::build_statement_context;

/// Pre-computed context shared across templates during code emission.
///
/// Built once from an [`Input`] and passed to each
/// [`Template`](crate::template::Template). Contains pre-built patterns,
/// wrapper detection, and per-statement contexts.
pub struct DeriveContext<'ir, L: Layout> {
    /// The original parsed input from the derive macro invocation.
    pub input: &'ir Input<L>,
    /// Extracted metadata (name, generics, crate path) for path construction.
    pub meta: InputMeta,
    /// Per-statement contexts keyed by statement/variant name, in declaration order.
    pub statements: IndexMap<String, StatementContext<'ir, L>>,
}

impl<'ir, L: Layout> DeriveContext<'ir, L> {
    /// Build a derive context from the parsed input.
    ///
    /// Extracts [`InputMeta`] and pre-computes a [`StatementContext`] for each
    /// statement (struct) or variant (enum) in declaration order.
    pub fn new(input: &'ir Input<L>) -> Self {
        let meta = InputMeta::from_input(input);
        let mut statements = IndexMap::new();

        match &input.data {
            crate::ir::Data::Struct(data) => {
                let stmt = &data.0;
                let ctx = build_statement_context(stmt);
                statements.insert(stmt.name.to_string(), ctx);
            }
            crate::ir::Data::Enum(data) => {
                for stmt in &data.variants {
                    let ctx = build_statement_context(stmt);
                    statements.insert(stmt.name.to_string(), ctx);
                }
            }
        }

        Self {
            input,
            meta,
            statements,
        }
    }
}

impl<L: Layout> ToTokens for DeriveContext<'_, L> {
    fn to_tokens(&self, _tokens: &mut TokenStream) {}
}
