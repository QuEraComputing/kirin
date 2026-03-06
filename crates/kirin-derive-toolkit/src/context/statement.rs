use crate::ir::fields::Wrapper;
use crate::ir::{Layout, Statement};
use crate::tokens::Pattern;

/// Pre-computed context for a single statement/variant.
///
/// Includes the destructuring [`Pattern`], wrapper status,
/// and pre-built wrapper access tokens, ready for use in match arms.
pub struct StatementContext<'ir, L: Layout> {
    /// Reference to the original IR statement.
    pub stmt: &'ir Statement<L>,
    /// Destructuring pattern for match arms (named or positional).
    pub pattern: Pattern,
    /// Whether this statement has a `#[wraps]` attribute.
    pub is_wrapper: bool,
    /// The wrapper metadata, if `#[wraps]` is present.
    pub wrapper: Option<&'ir Wrapper>,
    /// The Rust type of the wrapped value (e.g., `InnerOp`), if `#[wraps]` is present.
    pub wrapper_type: Option<&'ir syn::Type>,
    /// Token expression to access the wrapper field binding (e.g., `inner` or `field_0`).
    pub wrapper_binding: Option<proc_macro2::TokenStream>,
}

pub(crate) fn build_statement_context<'ir, L: Layout>(
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
