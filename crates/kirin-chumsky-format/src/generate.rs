//! Code generation for chumsky derive macros.

mod ast;
mod emit_ir;
mod parser;
mod pretty_print;

#[cfg(test)]
mod tests;

use std::collections::HashSet;

use kirin_derive_core::codegen::GenericsBuilder;
use kirin_derive_core::ir::VariantRef;
use proc_macro2::TokenStream;
use quote::quote;

use crate::ChumskyLayout;
use crate::field_kind::{ValueTypeScanner, collect_fields, fields_in_format};
use crate::format::Format;

pub use self::ast::GenerateAST;
pub use self::emit_ir::GenerateEmitIR;
pub use self::parser::GenerateHasDialectParser;
pub use self::pretty_print::GeneratePrettyPrint;

/// Shared configuration for code generators.
///
/// This extracts common paths from the IR input that all generators need.
#[derive(Clone)]
pub(crate) struct GeneratorConfig {
    /// Path to the kirin-chumsky crate (e.g., `::kirin::parsers`)
    pub crate_path: syn::Path,
    /// Path to the kirin IR crate (e.g., `::kirin::ir`)
    pub ir_path: syn::Path,
    /// The type lattice path (e.g., `SimpleType`)
    pub type_lattice: syn::Path,
}

impl GeneratorConfig {
    /// Creates a new generator config from IR input.
    pub fn new(ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>) -> Self {
        let crate_path = ir_input
            .extra_attrs
            .crate_path
            .clone()
            .or(ir_input.attrs.crate_path.clone())
            .unwrap_or_else(|| syn::parse_quote!(::kirin::parsers));
        let ir_path = ir_input
            .attrs
            .crate_path
            .clone()
            .unwrap_or_else(|| syn::parse_quote!(::kirin::ir));
        let type_lattice = ir_input.attrs.type_lattice.clone();
        Self {
            crate_path,
            ir_path,
            type_lattice,
        }
    }

    /// Builds AST generics with Language parameter.
    pub fn build_ast_generics(
        &self,
        ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>,
    ) -> syn::Generics {
        GenericsBuilder::new(&self.ir_path).with_language(&ir_input.generics)
    }
}

/// Gets the format string for a statement, checking extra_attrs first.
pub(crate) fn format_for_statement(
    ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>,
    stmt: &kirin_derive_core::ir::Statement<ChumskyLayout>,
) -> Option<String> {
    stmt.extra_attrs
        .format
        .clone()
        .or(stmt.attrs.format.clone())
        .or(ir_input.extra_attrs.format.clone())
}

/// Gets the set of field indices that are in the format string.
///
/// If there's no format string (e.g., wrapper variants), includes all fields.
pub(crate) fn get_fields_in_format(
    ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>,
    stmt: &kirin_derive_core::ir::Statement<ChumskyLayout>,
) -> HashSet<usize> {
    let Some(format_str) = format_for_statement(ir_input, stmt) else {
        return collect_fields(stmt).iter().map(|f| f.index).collect();
    };

    match Format::parse(&format_str, None) {
        Ok(format) => fields_in_format(&format, stmt),
        Err(_) => collect_fields(stmt).iter().map(|f| f.index).collect(),
    }
}

/// Collects all Value field types that contain type parameters from all statements.
///
/// Uses the `Scan` visitor pattern from `kirin-derive-core` to traverse the IR.
/// These types need trait bounds (e.g., `HasParser`, `EmitIR`, `PrettyPrint`) in generated impls.
pub(crate) fn collect_all_value_types_needing_bounds(
    ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>,
) -> Vec<syn::Type> {
    ValueTypeScanner::new(&ir_input.generics)
        .scan(ir_input)
        .unwrap_or_default()
}

/// Filters collected fields to only include those needed in the AST.
///
/// Fields are included if they:
/// - Are in the format string (will be parsed), OR
/// - Don't have a default value (required)
///
/// This is used by both AST generation and EmitIR generation.
pub(crate) fn filter_ast_fields<'a>(
    collected: &'a [crate::field_kind::CollectedField],
    fields_in_format: &HashSet<usize>,
) -> Vec<&'a crate::field_kind::CollectedField> {
    collected
        .iter()
        .filter(|f| fields_in_format.contains(&f.index) || f.default.is_none())
        .collect()
}

// =============================================================================
// Variant Iteration Helpers
// =============================================================================

/// Generates match arms for an enum, handling both wrapper and regular variants.
///
/// Uses `DataEnum::iter_variants()` from `kirin-derive-core` for variant classification.
///
/// - `type_name`: The enum type name (used in patterns like `TypeName::Variant`)
/// - `data`: The enum data containing variants
/// - `wrapper_handler`: Closure that generates code for wrapper variants
/// - `regular_handler`: Closure that generates code for regular variants
/// - `marker_handler`: Optional closure for the `__Marker` variant (for AST enums)
pub(crate) fn generate_enum_match<F, G>(
    type_name: &syn::Ident,
    data: &kirin_derive_core::ir::DataEnum<ChumskyLayout>,
    wrapper_handler: F,
    regular_handler: G,
    marker_handler: Option<TokenStream>,
) -> TokenStream
where
    F: Fn(&syn::Ident, &kirin_derive_core::ir::fields::Wrapper) -> TokenStream,
    G: Fn(&syn::Ident, &kirin_derive_core::ir::Statement<ChumskyLayout>) -> TokenStream,
{
    let arms: Vec<TokenStream> = data
        .iter_variants()
        .map(|variant| match variant {
            VariantRef::Wrapper { name, wrapper, .. } => {
                let body = wrapper_handler(name, wrapper);
                quote! { #type_name::#name(inner) => { #body } }
            }
            VariantRef::Regular { name, stmt } => regular_handler(name, stmt),
        })
        .collect();

    if let Some(marker) = marker_handler {
        quote! {
            match self {
                #(#arms)*
                #marker
            }
        }
    } else {
        quote! {
            match self {
                #(#arms)*
            }
        }
    }
}
