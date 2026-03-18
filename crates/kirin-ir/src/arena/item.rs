/// an arena item, provides generic interface
/// to the item Id and whether it has been marked as deleted.
#[derive(Debug, Clone)]
pub struct Item<T> {
    pub(crate) deleted: bool,
    pub(crate) data: T,
}

impl<T> Item<T> {
    pub(crate) fn new(data: T) -> Self {
        Self {
            data,
            deleted: false,
        }
    }
}

impl<T> Item<T> {
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
