/// annotation provides a way to attach metadata to arena items
/// it handles the case where some of the arena items has metadata and some do not
/// by using an `Option` internally
use crate::arena::Identifier;

#[derive(Debug, Clone)]
pub struct DenseHint<I: Identifier, T> {
    data: Vec<Option<T>>,
    marker: std::marker::PhantomData<I>,
}

impl<T: Clone, I: Identifier> DenseHint<I, T> {
    pub(crate) fn from_arena<U>(arena: &crate::arena::Arena<I, U>) -> Self {
        Self {
            data: vec![None; arena.len()],
            marker: std::marker::PhantomData,
        }
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn get(&self, id: I) -> Option<&T> {
        self.data.get(id.into().raw()).and_then(|opt| opt.as_ref())
    }

    pub fn get_mut(&mut self, id: I) -> Option<&mut T> {
        self.data
            .get_mut(id.into().raw())
            .and_then(|opt| opt.as_mut())
    }

    pub fn insert(&mut self, id: I, value: T) {
        if let Some(slot) = self.data.get_mut(id.into().raw()) {
            *slot = Some(value);
        }
    }

    pub fn insert_or_combine(&mut self, id: I, value: T, combine: impl FnOnce(&T, T) -> T) {
        let entry = self.data.get_mut(id.into().raw());
        if let Some(slot) = entry {
            match slot {
                Some(existing) => {
                    let new_value = combine(existing, value);
                    *existing = new_value;
                }
                None => {
                    *slot = Some(value);
                }
            }
        }
    }
}

impl<I, T> std::ops::Index<I> for DenseHint<I, T>
where
    I: Identifier,
    T: Clone,
{
    type Output = T;

    fn index(&self, index: I) -> &Self::Output {
        self.get(index)
            .expect("No annotation found for the given identifier")
    }
}

impl<I, T> std::ops::IndexMut<I> for DenseHint<I, T>
where
    I: Identifier,
    T: Clone,
{
    fn index_mut(&mut self, index: I) -> &mut Self::Output {
        self.get_mut(index)
            .expect("No annotation found for the given identifier")
    }
}
