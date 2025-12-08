use super::data::Arena;
use super::id::{Id, Identifier};

/// IdMap from old -> new
pub struct IdMap<I: Identifier>(Vec<Option<I>>);

impl<I: Identifier> IdMap<I> {
    pub fn get(&self, old: I) -> Option<I> {
        if old.into().raw() > self.0.len() {
            panic!("unexpected Id")
        }
        self.0[old.into().raw()]
    }
}

impl<I: Identifier, T> Arena<I, T> {
    pub fn gc(&mut self) -> IdMap<I> {
        let mut counter = 0;
        let raw = self
            .items
            .iter()
            .map(|item| {
                if item.deleted() {
                    None
                } else {
                    counter += 1;
                    Some(I::from(Id(counter - 1)))
                }
            })
            .collect::<Vec<_>>();
        self.items.retain(|item| !item.deleted());
        return IdMap(raw);
    }
}
