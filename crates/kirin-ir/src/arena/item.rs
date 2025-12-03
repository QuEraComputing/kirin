/// an arena item, provides generic interface
/// to the item Id and whether it has been marked as deleted.
#[derive(Debug, Clone)]
pub struct Item<T> {
    pub(super) deleted: bool,
    pub(super) data: T,
}

#[bon::bon]
impl<T> Item<T> {
    #[builder]
    pub fn new(data: T, deleted: Option<bool>) -> Self {
        Self {
            data,
            deleted: deleted.unwrap_or(false),
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
