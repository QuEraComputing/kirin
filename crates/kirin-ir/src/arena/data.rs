use super::id::{Id, Identifier};
use super::item::Item;

#[derive(Debug, Clone)]
pub struct Arena<T, I: Identifier> {
    pub(super) items: Vec<Item<T>>,
    marker: std::marker::PhantomData<I>,
}

impl<T, I: Identifier> Default for Arena<T, I> {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            marker: std::marker::PhantomData,
        }
    }
}

impl<T, I: Identifier> Arena<T, I> {
    pub fn next_id(&self) -> I {
        I::from(Id(self.items.len()))
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Allocate a new item in the arena and return its identifier.
    pub fn alloc(&mut self, item: T) -> I {
        let id = self.next_id();
        self.items
            .push(Item::builder().data(item).deleted(false).build());
        id
    }

    pub fn get(&self, id: impl Into<I>) -> Option<&Item<T>> {
        self.items.get(id.into().into().raw())
    }

    pub fn get_mut(&mut self, id: impl Into<I>) -> Option<&mut Item<T>> {
        self.items.get_mut(id.into().into().raw())
    }

    pub fn delete(&mut self, id: impl Into<I>) -> bool {
        if let Some(arena_item) = self.get_mut(id) {
            if !arena_item.deleted {
                arena_item.deleted = true;
                return true;
            }
        }
        false
    }

    pub fn iter(&self) -> impl Iterator<Item = &Item<T>> {
        self.items
            .iter()
            .filter(|arena_item| !arena_item.deleted)
            .map(|arena_item| arena_item)
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Item<T>> {
        self.items
            .iter_mut()
            .filter(|arena_item| !arena_item.deleted)
            .map(|arena_item| arena_item)
    }
}

impl<T, I: Identifier> std::ops::Index<I> for Arena<T, I> {
    type Output = Item<T>;

    fn index(&self, index: I) -> &Self::Output {
        &self.items[index.into().raw()]
    }
}

impl<T, I: Identifier> std::ops::IndexMut<I> for Arena<T, I> {
    fn index_mut(&mut self, index: I) -> &mut Self::Output {
        &mut self.items[index.into().raw()]
    }
}
