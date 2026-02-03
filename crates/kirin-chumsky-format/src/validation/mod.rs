//! Validation for format strings and field usage.
//!
//! This module provides `ValidationVisitor` which validates that:
//! - No fields use Vec or Option collection types
//! - All field references in the format string are valid
//! - :name/:type options are only used on SSA/Result fields
//! - No duplicate default occurrences for the same field
//! - All required fields are mentioned in the format string
//! - SSA/Result fields have at least a name occurrence

mod result;
mod visitor;

pub use result::{FieldOccurrence, ValidationResult};
pub use visitor::ValidationVisitor;

use kirin_derive_core::ir::Statement;

use crate::ChumskyLayout;
use kirin_derive_core::ir::fields::FieldInfo;
use crate::format::Format;

/// Validates a format string against collected fields.
///
/// This is a convenience function that creates a ValidationVisitor
/// and runs the validation.
pub fn validate_format<'ir>(
    stmt: &'ir Statement<ChumskyLayout>,
    format: &Format<'_>,
    collected: &'ir [FieldInfo<ChumskyLayout>],
) -> syn::Result<ValidationResult<'ir>> {
    ValidationVisitor::new().validate(stmt, format, collected)
}
