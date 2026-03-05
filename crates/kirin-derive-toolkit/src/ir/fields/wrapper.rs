use super::index::FieldIndex;

/// Metadata for a `#[wraps]` delegation field.
///
/// When an enum variant has `#[wraps]`, it delegates to an inner type
/// (usually another dialect's statement). The wrapper tracks which field
/// holds the inner type and its `syn::Type`.
#[derive(Debug, Clone)]
pub struct Wrapper {
    pub field: FieldIndex,
    pub ty: syn::Type,
}

impl Wrapper {
    pub fn new(index: usize, f: &syn::Field) -> Self {
        Self {
            field: FieldIndex::new(f.ident.clone(), index),
            ty: f.ty.clone(),
        }
    }
}
