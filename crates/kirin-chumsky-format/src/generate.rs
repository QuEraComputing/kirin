//! Code generation for chumsky derive macros.

mod ast;
mod emit_ir;
mod parser;
mod pretty_print;

#[cfg(test)]
mod tests;

use std::collections::HashSet;

use quote::quote;

use crate::ChumskyLayout;
use crate::field_kind::{collect_fields, collect_value_types_with_type_params, fields_in_format};
use crate::format::Format;
use crate::generics::GenericsBuilder;

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
/// These types need trait bounds (e.g., `HasParser`, `EmitIR`, `PrettyPrint`) in generated impls.
pub(crate) fn collect_all_value_types_needing_bounds(
    ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>,
) -> Vec<syn::Type> {
    let mut all_types = Vec::new();

    match &ir_input.data {
        kirin_derive_core::ir::Data::Struct(s) => {
            let collected = collect_fields(&s.0);
            all_types.extend(collect_value_types_with_type_params(
                &collected,
                &ir_input.generics,
            ));
        }
        kirin_derive_core::ir::Data::Enum(e) => {
            for variant in e.variants.iter() {
                let collected = collect_fields(variant);
                all_types.extend(collect_value_types_with_type_params(
                    &collected,
                    &ir_input.generics,
                ));
            }
        }
    }

    // Deduplicate
    let mut seen = std::collections::HashSet::new();
    all_types.retain(|ty| {
        let key = quote!(#ty).to_string();
        seen.insert(key)
    });

    all_types
}

/// Deduplicates a list of types by their token representation.
pub(crate) fn deduplicate_types(types: &mut Vec<syn::Type>) {
    let mut seen = std::collections::HashSet::new();
    types.retain(|ty| {
        let key = quote!(#ty).to_string();
        seen.insert(key)
    });
}
