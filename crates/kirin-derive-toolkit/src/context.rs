use indexmap::IndexMap;

use crate::derive::InputMeta;
use crate::ir::{Input, Layout, Statement, fields::Wrapper};
use crate::tokens::Pattern;

/// Pre-computed context shared across generators during code emission.
///
/// Built once from an [`Input`] and passed to each
/// [`Generator`](crate::generator::Generator). Contains pre-built patterns,
/// wrapper detection, and per-statement contexts to avoid repeated scanning.
pub struct DeriveContext<'ir, L: Layout> {
    pub input: &'ir Input<L>,
    pub meta: InputMeta,
    pub statements: IndexMap<String, StatementContext<'ir, L>>,
}

/// Pre-computed context for a single statement/variant.
///
/// Includes the destructuring [`Pattern`], wrapper status,
/// and pre-built wrapper access tokens, ready for use in match arms.
pub struct StatementContext<'ir, L: Layout> {
    pub stmt: &'ir Statement<L>,
    pub pattern: Pattern,
    pub is_wrapper: bool,
    pub wrapper: Option<&'ir Wrapper>,
    /// The Rust type of the wrapped value (e.g., `InnerOp`), if `#[wraps]` is present.
    pub wrapper_type: Option<&'ir syn::Type>,
    /// Token expression to access the wrapper field binding (e.g., `inner` or `field_0`).
    pub wrapper_binding: Option<proc_macro2::TokenStream>,
}

impl<'ir, L: Layout> DeriveContext<'ir, L> {
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

fn build_statement_context<'ir, L: Layout>(stmt: &'ir Statement<L>) -> StatementContext<'ir, L> {
    let is_wrapper = stmt.wraps.is_some();
    let wrapper = stmt.wraps.as_ref();

    let mut all_fields = Vec::new();
    if let Some(w) = &stmt.wraps {
        all_fields.push(crate::ir::fields::FieldIndex::new(
            w.field.ident.clone(),
            w.field.index,
        ));
    }
    for f in stmt.iter_all_fields() {
        all_fields.push(crate::ir::fields::FieldIndex::new(f.ident.clone(), f.index));
    }
    all_fields.sort_by_key(|field| field.index);

    let pattern = if all_fields.is_empty() {
        Pattern::new(false, Vec::new())
    } else {
        let named = all_fields.iter().any(|field| field.ident.is_some());
        let names: Vec<proc_macro2::TokenStream> = all_fields
            .iter()
            .map(|field| {
                let name = field.name();
                quote::quote! { #name }
            })
            .collect();
        Pattern::new(named, names)
    };

    let wrapper_type = stmt.wraps.as_ref().map(|w| &w.ty);
    let wrapper_binding = stmt.wraps.as_ref().map(|w| {
        let name = w.field.name();
        quote::quote! { #name }
    });

    StatementContext {
        stmt,
        pattern,
        is_wrapper,
        wrapper,
        wrapper_type,
        wrapper_binding,
    }
}
