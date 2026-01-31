use super::{collection::Collection, index::FieldIndex};

#[derive(Debug, Clone, Default)]
pub struct Blocks {
    data: Vec<Block>,
}

#[derive(Debug, Clone)]
pub struct Block {
    pub field: FieldIndex,
    pub collection: Collection,
}

impl Blocks {
    pub fn add(&mut self, index: usize, f: &syn::Field) -> darling::Result<bool> {
        let Some(coll) = Collection::from_type(&f.ty, "Block") else {
            return Ok(false);
        };
        self.data.push(Block {
            field: FieldIndex::new(f.ident.clone(), index),
            collection: coll,
        });
        Ok(true)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Block> {
        self.data.iter()
    }
}
