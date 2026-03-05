use indexmap::IndexMap;

use crate::derive::InputMeta;
use crate::ir::{Input, Layout, Statement, fields::Wrapper};
use crate::tokens::Pattern;

pub struct DeriveContext<'ir, L: Layout> {
    pub input: &'ir Input<L>,
    pub meta: InputMeta,
    pub statements: IndexMap<String, StatementContext<'ir, L>>,
}

pub struct StatementContext<'ir, L: Layout> {
    pub stmt: &'ir Statement<L>,
    pub pattern: Pattern,
    pub is_wrapper: bool,
    pub wrapper: Option<&'ir Wrapper>,
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

fn build_statement_context<'ir, L: Layout>(
    stmt: &'ir Statement<L>,
) -> StatementContext<'ir, L> {
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

    StatementContext {
        stmt,
        pattern,
        is_wrapper,
        wrapper,
    }
}
