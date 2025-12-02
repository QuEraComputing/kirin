use super::id::Id;

pub struct Arena<T>(pub(crate) Vec<Item<T>>);

pub struct Item<T> {
    id: Id,
    deleted: bool,
    data: T,
}

#[bon::bon]
impl<T> Item<T> {
    #[builder]
    pub fn new(id: Id, data: T, deleted: Option<bool>) -> Self {
        Self {
            id,
            data,
            deleted: deleted.unwrap_or(false),
        }
    }

    pub fn unwrap(self) -> T {
        self.data
    }

    pub fn deleted(&self) -> bool {
        self.deleted
    }
}

impl<T> std::ops::Deref for Item<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl<T> std::ops::DerefMut for Item<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data
    }
}

impl<T> Default for Arena<T> {
    fn default() -> Self {
        Self(Vec::new())
    }
}

impl<T> Arena<T> {
    pub fn next_id(&self) -> Id {
        Id(self.0.len())
    }

    pub fn insert(&mut self, item: T) -> Id {
        let id = self.next_id();
        self.0.push(Item {
            id,
            data: item,
            deleted: false,
        });
        id
    }

    pub fn get(&self, id: Id) -> Option<&Item<T>> {
        self.0.get(id.raw())
    }

    pub fn get_mut(&mut self, id: Id) -> Option<&mut Item<T>> {
        self.0.get_mut(id.raw())
    }

    pub fn delete(&mut self, id: Id) -> bool {
        if let Some(arena_item) = self.get_mut(id) {
            if arena_item.id == id && !arena_item.deleted {
                arena_item.deleted = true;
                return true;
            }
        }
        false
    }

    pub fn iter(&self) -> impl Iterator<Item = &Item<T>> {
        self.0
            .iter()
            .filter(|arena_item| !arena_item.deleted)
            .map(|arena_item| arena_item)
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Item<T>> {
        self.0
            .iter_mut()
            .filter(|arena_item| !arena_item.deleted)
            .map(|arena_item| arena_item)
    }
}
