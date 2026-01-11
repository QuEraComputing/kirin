use super::{collection::Collection, index::FieldIndex};

#[derive(Debug, Clone, Default)]
pub struct Regions {
    data: Vec<Region>,
}

impl Regions {
    pub fn add(&mut self, index: usize, f: &syn::Field) -> darling::Result<bool> {
        let Some(coll) = Collection::from_type(&f.ty, "Region") else {
            return Ok(false);
        };
        self.data.push(Region {
            field: FieldIndex::new(f.ident.clone(), index),
            collection: coll,
        });
        Ok(true)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Region> {
        self.data.iter()
    }
}

#[derive(Debug, Clone)]
pub struct Region {
    pub field: FieldIndex,
    pub collection: Collection,
}
