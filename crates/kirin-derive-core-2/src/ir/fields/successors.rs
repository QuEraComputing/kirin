use super::{collection::Collection, index::FieldIndex};

#[derive(Debug, Clone, Default)]
pub struct Successors {
    data: Vec<Successor>,
}

impl Successors {
    pub fn add(&mut self, index: usize, f: &syn::Field) -> darling::Result<bool> {
        let Some(coll) = Collection::from_type(&f.ty, "Successor") else {
            return Ok(false);
        };
        self.data.push(Successor {
            field: FieldIndex::new(f.ident.clone(), index),
            collection: coll,
        });
        Ok(true)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Successor> {
        self.data.iter()
    }
}

#[derive(Debug, Clone)]
pub struct Successor {
    pub field: FieldIndex,
    pub collection: Collection,
}
