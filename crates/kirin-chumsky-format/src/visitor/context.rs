use std::collections::HashMap;

use kirin_derive_core::ir::Statement;

use crate::ChumskyLayout;
use crate::format::Format;
use kirin_derive_core::ir::fields::FieldInfo;

use super::format_visitor::build_field_map;

/// Context passed to visitors that need access to the statement and format.
///
/// This is useful for visitors that need to look up additional information
/// during traversal.
pub struct VisitorContext<'ir, 'fmt> {
    /// The statement being visited
    pub stmt: &'ir Statement<ChumskyLayout>,
    /// The parsed format string
    pub format: &'fmt Format<'fmt>,
    /// Map from field name/index to field
    pub field_map: HashMap<String, &'ir FieldInfo<ChumskyLayout>>,
}

impl<'ir, 'fmt> VisitorContext<'ir, 'fmt> {
    /// Creates a new visitor context.
    pub fn new(
        stmt: &'ir Statement<ChumskyLayout>,
        format: &'fmt Format<'fmt>,
        collected: &'ir [FieldInfo<ChumskyLayout>],
    ) -> Self {
        Self {
            stmt,
            format,
            field_map: build_field_map(stmt, collected),
        }
    }

    /// Looks up a field by name or index.
    pub fn get_field(&self, name: &str) -> Option<&'ir FieldInfo<ChumskyLayout>> {
        self.field_map.get(name).copied()
    }
}
