use super::id::{Id, Identifier};
use super::item::Item;

#[derive(Debug, Clone)]
pub struct Arena<I: Identifier, T> {
    pub(crate) items: Vec<Item<T>>,
    marker: std::marker::PhantomData<I>,
}

impl<I: Identifier, T> Default for Arena<I, T> {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            marker: std::marker::PhantomData,
        }
    }
}

impl<I: Identifier, T> Arena<I, T> {
    pub fn next_id(&self) -> I {
        I::from(Id(self.items.len()))
    }

    /// Returns the total number of slots (including deleted tombstones).
    /// Use `iter()` to get only live items.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Allocate a new item in the arena and return its identifier.
    #[must_use]
    pub fn alloc(&mut self, item: T) -> I {
        let id = self.next_id();
        self.items.push(Item::new(item));
        id
    }

    #[must_use]
    pub fn alloc_with_id(&mut self, f: impl FnOnce(I) -> T) -> I {
        let id = self.next_id();
        let item = f(id);
        self.items.push(Item::new(item));
        id
    }

    pub fn get(&self, id: impl Into<I>) -> Option<&Item<T>> {
        self.items.get(id.into().into().raw())
    }

    pub fn get_mut(&mut self, id: impl Into<I>) -> Option<&mut Item<T>> {
        self.items.get_mut(id.into().into().raw())
    }

    #[must_use]
    pub fn delete(&mut self, id: impl Into<I>) -> bool {
        if let Some(arena_item) = self.get_mut(id)
            && !arena_item.deleted
        {
            arena_item.deleted = true;
            return true;
        }
        false
    }

    pub fn iter(&self) -> impl Iterator<Item = &Item<T>> {
        self.items.iter().filter(|arena_item| !arena_item.deleted)
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Item<T>> {
        self.items
            .iter_mut()
            .filter(|arena_item| !arena_item.deleted)
    }

    /// Map all live items, skipping deleted tombstones.
    /// Returns a new arena preserving layout and indices.
    /// Deleted items are preserved as deleted tombstones with default data.
    pub fn try_map_live<U: Default, E>(
        self,
        mut f: impl FnMut(T) -> Result<U, E>,
    ) -> Result<Arena<I, U>, E> {
        let items = self
            .items
            .into_iter()
            .map(|item| {
                if item.deleted {
                    Ok(Item {
                        deleted: true,
                        data: U::default(),
                    })
                } else {
                    Ok(Item {
                        deleted: false,
                        data: f(item.data)?,
                    })
                }
            })
            .collect::<Result<Vec<_>, E>>()?;
        Ok(Arena {
            items,
            marker: std::marker::PhantomData,
        })
    }

    /// Fallibly map all live items, using an `Option`-based tombstone for deleted items.
    /// Deleted items become `None` tombstones, live items become `Some(f(data))`.
    /// This avoids requiring `Default` on the output type.
    pub fn try_map_live_option<U, E>(
        self,
        mut f: impl FnMut(T) -> Result<U, E>,
    ) -> Result<Arena<I, Option<U>>, E> {
        let items = self
            .items
            .into_iter()
            .map(|item| {
                if item.deleted {
                    Ok(Item {
                        deleted: true,
                        data: None,
                    })
                } else {
                    Ok(Item {
                        deleted: false,
                        data: Some(f(item.data)?),
                    })
                }
            })
            .collect::<Result<Vec<_>, E>>()?;
        Ok(Arena {
            items,
            marker: std::marker::PhantomData,
        })
    }

    /// Map live items with `f`, deleted items with `tombstone`.
    /// Returns a new arena preserving layout and indices.
    pub fn map_live<U>(
        self,
        mut f: impl FnMut(T) -> U,
        mut tombstone: impl FnMut(T) -> U,
    ) -> Arena<I, U> {
        let items = self
            .items
            .into_iter()
            .map(|item| Item {
                deleted: item.deleted,
                data: if item.deleted {
                    tombstone(item.data)
                } else {
                    f(item.data)
                },
            })
            .collect();
        Arena {
            items,
            marker: std::marker::PhantomData,
        }
    }

    /// Map all items (live and deleted) infallibly.
    /// Returns a new arena preserving layout and indices.
    pub fn map<U>(self, mut f: impl FnMut(T) -> U) -> Arena<I, U> {
        let items = self
            .items
            .into_iter()
            .map(|item| Item {
                deleted: item.deleted,
                data: f(item.data),
            })
            .collect();
        Arena {
            items,
            marker: std::marker::PhantomData,
        }
    }
}

impl<T, I: Identifier> std::ops::Index<I> for Arena<I, T> {
    type Output = Item<T>;

    fn index(&self, index: I) -> &Self::Output {
        &self.items[index.into().raw()]
    }
}

impl<T, I: Identifier> std::ops::IndexMut<I> for Arena<I, T> {
    fn index_mut(&mut self, index: I) -> &mut Self::Output {
        &mut self.items[index.into().raw()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // A minimal identifier for testing.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    struct TestId(Id);

    impl From<Id> for TestId {
        fn from(id: Id) -> Self {
            TestId(id)
        }
    }

    impl From<TestId> for Id {
        fn from(id: TestId) -> Self {
            id.0
        }
    }

    impl Identifier for TestId {}

    #[test]
    fn arena_alloc_and_get() {
        let mut arena: Arena<TestId, i32> = Arena::default();
        let id = arena.alloc(42);
        assert_eq!(**arena.get(id).unwrap(), 42);
        assert_eq!(arena.len(), 1);
    }

    #[test]
    fn arena_delete_tombstones_excluded_from_iter() {
        let mut arena: Arena<TestId, &str> = Arena::default();
        let a = arena.alloc("a");
        let _b = arena.alloc("b");
        let c = arena.alloc("c");

        assert!(arena.delete(a));
        // DESIGN NOTE: Arena::len() includes tombstones. A live_count() method would be useful.
        assert_eq!(arena.len(), 3);

        let live: Vec<&str> = arena.iter().map(|item| **item).collect();
        assert_eq!(live, vec!["b", "c"]);

        // get() still returns deleted items (with deleted flag)
        assert!(arena.get(a).unwrap().deleted());
        assert!(!arena.get(c).unwrap().deleted());
    }

    #[test]
    fn arena_delete_returns_false_for_already_deleted() {
        let mut arena: Arena<TestId, i32> = Arena::default();
        let id = arena.alloc(1);
        assert!(arena.delete(id));
        assert!(!arena.delete(id), "second delete should return false");
    }

    #[test]
    fn arena_alloc_with_id() {
        let mut arena: Arena<TestId, (TestId, String)> = Arena::default();
        let id = arena.alloc_with_id(|id| (id, "self-referential".to_string()));
        let item = arena.get(id).unwrap();
        assert_eq!(item.0, id);
        assert_eq!(item.1, "self-referential");
    }

    #[test]
    fn arena_is_empty() {
        let mut arena: Arena<TestId, i32> = Arena::default();
        assert!(arena.is_empty());
        arena.alloc(1);
        assert!(!arena.is_empty());
    }

    #[test]
    fn arena_next_id_increments() {
        let mut arena: Arena<TestId, i32> = Arena::default();
        let id0 = arena.next_id();
        arena.alloc(10);
        let id1 = arena.next_id();
        assert_ne!(id0, id1);
        assert_eq!(Id::from(id0).raw(), 0);
        assert_eq!(Id::from(id1).raw(), 1);
    }
}
