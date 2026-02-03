//! Validation result types.

use crate::field_kind::CollectedField;
use crate::format::FormatOption;

/// Result of validation containing field occurrences.
#[derive(Debug)]
pub struct ValidationResult<'a> {
    /// Field occurrences in format string order
    pub occurrences: Vec<FieldOccurrence<'a>>,
}

/// Represents an occurrence of a field in the format string.
#[derive(Debug, Clone)]
pub struct FieldOccurrence<'a> {
    /// The collected field info.
    pub field: &'a CollectedField,
    /// The format option for this occurrence.
    pub option: FormatOption,
    /// The unique variable name for this occurrence.
    pub var_name: syn::Ident,
}
