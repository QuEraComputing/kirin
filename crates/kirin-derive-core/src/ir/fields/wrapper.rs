use super::index::FieldIndex;

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
