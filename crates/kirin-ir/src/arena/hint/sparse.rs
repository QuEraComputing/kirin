use rustc_hash::FxHashMap;

use crate::Identifier;

pub struct SparseHint<I: Identifier, T> {
    data: FxHashMap<I, T>,
}

impl<I: Identifier, T> Default for SparseHint<I, T> {
    fn default() -> Self {
        Self {
            data: FxHashMap::default(),
        }
    }
}

impl<I: Identifier, T> SparseHint<I, T> {
    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    pub fn get(&self, id: I) -> Option<&T> {
        self.data.get(&id)
    }

    pub fn get_mut(&mut self, id: I) -> Option<&mut T> {
        self.data.get_mut(&id)
    }

    pub fn insert(&mut self, id: I, value: T) {
        self.data.insert(id, value);
    }

    pub fn insert_or_combine(&mut self, id: I, value: T, combine: impl FnOnce(&T, T) -> T) {
        use std::collections::hash_map::Entry;
        match self.data.entry(id) {
            Entry::Occupied(mut entry) => {
                let new_value = combine(entry.get(), value);
                *entry.get_mut() = new_value;
            }
            Entry::Vacant(entry) => {
                entry.insert(value);
            }
        }
    }
}

impl<T, I> std::ops::Index<I> for SparseHint<I, T>
where
    I: Identifier,
{
    type Output = T;

    fn index(&self, index: I) -> &Self::Output {
        self.get(index)
            .expect("No hint found for the given identifier")
    }
}

impl<T, I> std::ops::IndexMut<I> for SparseHint<I, T>
where
    I: Identifier,
{
    fn index_mut(&mut self, index: I) -> &mut Self::Output {
        self.get_mut(index)
            .expect("No hint found for the given identifier")
    }
}
