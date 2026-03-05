use std::collections::HashSet;

use kirin_derive_toolkit::ir::Layout;
use kirin_derive_toolkit::misc::is_type_in_generic;
use kirin_derive_toolkit::scan::Scan;
use quote::quote;

use crate::ChumskyLayout;
use crate::format::{Format, FormatElement};

/// Scanner that collects Value field types containing type parameters.
pub struct ValueTypeScanner<'a> {
    /// Type parameter names to check against
    type_param_names: Vec<String>,
    /// Collected types that need bounds
    types: Vec<syn::Type>,
    /// Set for deduplication
    seen: HashSet<String>,
    /// Reference to generics for lifetime
    _generics: &'a syn::Generics,
}

impl<'a> ValueTypeScanner<'a> {
    /// Creates a new scanner for the given generics.
    pub fn new(generics: &'a syn::Generics) -> Self {
        let type_param_names = generics
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

        Self {
            type_param_names,
            types: Vec::new(),
            seen: HashSet::new(),
            _generics: generics,
        }
    }

    /// Scans the input and returns collected types.
    pub fn scan(
        mut self,
        input: &kirin_derive_toolkit::ir::Input<ChumskyLayout>,
    ) -> darling::Result<Vec<syn::Type>> {
        kirin_derive_toolkit::scan::scan_input(&mut self, input)?;
        Ok(self.types)
    }

    /// Checks if a type contains any of our type parameters and adds it if so.
    fn maybe_add_type(&mut self, ty: &syn::Type, has_default: bool) {
        // Skip fields with defaults - they're not parsed
        if has_default {
            return;
        }

        // Check if any type parameter appears in this type
        for param_name in &self.type_param_names {
            if kirin_derive_toolkit::misc::is_type(ty, param_name.as_str())
                || is_type_in_generic(ty, param_name.as_str())
            {
                // Deduplicate
                let key = quote!(#ty).to_string();
                if self.seen.insert(key) {
                    self.types.push(ty.clone());
                }
                break;
            }
        }
    }
}

impl<'ir> Scan<'ir, ChumskyLayout> for ValueTypeScanner<'_> {
    fn scan_value(
        &mut self,
        field: &'ir kirin_derive_toolkit::ir::fields::FieldInfo<ChumskyLayout>,
    ) -> darling::Result<()> {
        if let Some(ty) = field.value_type() {
            self.maybe_add_type(ty, field.has_default());
        }
        Ok(())
    }
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
