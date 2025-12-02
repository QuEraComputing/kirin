use crate::Arena;
use crate::Id;

/// IdMap from old -> new
pub struct IdMap(Vec<Option<Id>>);

impl IdMap {
    pub fn get(&self, old: Id) -> Option<Id> {
        if old.raw() > self.0.len() {
            panic!("unexpected Id")
        }
        self.0[old.raw()]
    }
}

impl<T> Arena<T> {
    pub fn gc(&mut self) -> IdMap {
        let mut counter = 0;
        let raw = self
            .0
            .iter()
            .map(|item| {
                if item.deleted() {
                    None
                } else {
                    counter += 1;
                    Some(Id(counter - 1))
                }
            })
            .collect::<Vec<_>>();
        self.0.retain(|item| !item.deleted());
        return IdMap(raw);
    }
}
