/// annotation provides a way to attach metadata to arena items
/// it handles the case where some of the arena items has metadata and some do not
/// by using an `Option` internally
use crate::arena::Identifier;

#[derive(Debug, Clone)]
pub struct DenseHint<I: Identifier, T> {
    data: Vec<Option<T>>,
    marker: std::marker::PhantomData<I>,
}

impl<I: Identifier, T> DenseHint<I, T> {
    pub(crate) fn from_arena<U>(arena: &crate::arena::Arena<I, U>) -> Self {
        Self {
            data: std::iter::repeat_with(|| None).take(arena.len()).collect(),
            marker: std::marker::PhantomData,
        }
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
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
        let idx = id.into().raw();
        if idx >= self.data.len() {
            self.data.resize_with(idx + 1, || None);
        }
        self.data[idx] = Some(value);
    }

    pub fn insert_or_combine(&mut self, id: I, value: T, combine: impl FnOnce(&T, T) -> T) {
        let idx = id.into().raw();
        if idx >= self.data.len() {
            self.data.resize_with(idx + 1, || None);
        }
        match &mut self.data[idx] {
            Some(existing) => {
                let new_value = combine(existing, value);
                *existing = new_value;
            }
            slot @ None => {
                *slot = Some(value);
            }
        }
    }
}

impl<I, T> std::ops::Index<I> for DenseHint<I, T>
where
    I: Identifier,
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
{
    fn index_mut(&mut self, index: I) -> &mut Self::Output {
        self.get_mut(index)
            .expect("No annotation found for the given identifier")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arena::id::Id;

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

    fn make_hint(len: usize) -> DenseHint<TestId, i32> {
        DenseHint {
            data: std::iter::repeat_with(|| None).take(len).collect(),
            marker: std::marker::PhantomData,
        }
    }

    #[test]
    fn insert_or_combine_out_of_range_resizes() {
        let mut hint = make_hint(2);
        let out_of_range_id = TestId(Id(5));

        hint.insert_or_combine(out_of_range_id, 42, |existing, new| existing + new);

        assert_eq!(hint.get(out_of_range_id), Some(&42));
        assert_eq!(hint.len(), 6);
    }

    #[test]
    fn insert_or_combine_combines_existing() {
        let mut hint = make_hint(2);
        let id = TestId(Id(0));

        hint.insert(id, 10);
        hint.insert_or_combine(id, 5, |existing, new| existing + new);

        assert_eq!(hint.get(id), Some(&15));
    }

    #[test]
    fn insert_or_combine_inserts_into_empty_slot() {
        let mut hint = make_hint(2);
        let id = TestId(Id(1));

        hint.insert_or_combine(id, 7, |existing, new| existing + new);

        assert_eq!(hint.get(id), Some(&7));
    }
}
