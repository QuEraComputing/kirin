use std::collections::HashSet;

use kirin_derive_toolkit::ir::Layout;
use kirin_derive_toolkit::ir::fields::FieldCategory;
use kirin_derive_toolkit::misc::{is_type, is_type_in_generic};
use quote::quote;

use crate::ChumskyLayout;
use crate::format::{Format, FormatElement};

/// Collects Value field types that contain type parameters.
pub fn collect_value_types_needing_bounds(
    input: &kirin_derive_toolkit::ir::Input<ChumskyLayout>,
    generics: &syn::Generics,
) -> Vec<syn::Type> {
    let type_param_names: Vec<String> = generics
        .params
        .iter()
        .filter_map(|p| {
            if let syn::GenericParam::Type(tp) = p {
                Some(tp.ident.to_string())
            } else {
                None
            }
        })
        .collect();

    if type_param_names.is_empty() {
        return Vec::new();
    }

    let mut types = Vec::new();
    let mut seen = HashSet::new();

    let statements: Vec<&kirin_derive_toolkit::ir::Statement<ChumskyLayout>> = match &input.data {
        kirin_derive_toolkit::ir::Data::Struct(data) => vec![&data.0],
        kirin_derive_toolkit::ir::Data::Enum(data) => data.variants.iter().collect(),
    };

    for stmt in statements {
        let fields = stmt.collect_fields();
        for field in &fields {
            if field.category() != FieldCategory::Value {
                continue;
            }
            if let Some(ty) = field.value_type() {
                if field.has_default() {
                    continue;
                }
                for param_name in &type_param_names {
                    if is_type(ty, param_name) || is_type_in_generic(ty, param_name) {
                        let key = quote!(#ty).to_string();
                        if seen.insert(key) {
                            types.push(ty.clone());
                        }
                        break;
                    }
                }
            }
        }
    }

    types
}

/// Returns the set of field indices that are mentioned in the format string.
pub fn fields_in_format<L: Layout>(
    format: &Format<'_>,
    stmt: &kirin_derive_toolkit::ir::Statement<L>,
) -> HashSet<usize> {
    let map_by_ident = stmt.field_name_to_index();
    let mut indices = HashSet::new();

    for elem in format.elements() {
        if let FormatElement::Field(name, _) = elem {
            // Try to parse as index first, then look up by name
            let index = name
                .parse::<usize>()
                .ok()
                .or_else(|| map_by_ident.get(&name.to_string()).copied());
            if let Some(idx) = index {
                indices.insert(idx);
            }
        }
    }

    indices
}
