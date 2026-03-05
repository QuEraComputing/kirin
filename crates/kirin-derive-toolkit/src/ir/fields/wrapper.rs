use super::index::FieldIndex;

/// Metadata for a `#[wraps]` delegation field.
///
/// When an enum variant has `#[wraps]`, it delegates to an inner type
/// (usually another dialect's statement). The wrapper tracks which field
/// holds the inner type and its `syn::Type`.
#[derive(Debug, Clone)]
pub struct Wrapper {
    /// Position and name of the wrapped field.
    pub field: FieldIndex,
    /// The inner type being delegated to.
    pub ty: syn::Type,
}

impl Wrapper {
    /// Create a wrapper from a field's position and `syn::Field` definition.
    pub fn new(index: usize, f: &syn::Field) -> Self {
        Self {
            field: FieldIndex::new(f.ident.clone(), index),
            ty: f.ty.clone(),
        }
    }
}
