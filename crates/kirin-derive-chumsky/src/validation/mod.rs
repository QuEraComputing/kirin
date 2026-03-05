//! Validation for format strings and field usage.

mod result;
mod visitor;

pub use result::{FieldOccurrence, ValidationResult};
pub use visitor::ValidationVisitor;

use kirin_derive_toolkit::ir::Statement;

use crate::ChumskyLayout;
use crate::format::Format;
use kirin_derive_toolkit::ir::fields::FieldInfo;

/// Validates a format string against collected fields.
pub fn validate_format<'ir>(
    stmt: &'ir Statement<ChumskyLayout>,
    format: &Format<'_>,
    collected: &'ir [FieldInfo<ChumskyLayout>],
) -> syn::Result<ValidationResult<'ir>> {
    ValidationVisitor::new().validate(stmt, format, collected)
}
