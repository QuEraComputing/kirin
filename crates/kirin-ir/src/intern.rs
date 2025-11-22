pub trait InternKey:
    From<usize> + Into<usize> + Clone + Copy + PartialEq + Eq + std::hash::Hash
{
}

impl InternKey for usize {}

#[derive(Clone, Debug)]
pub struct InternTable<T: Clone + Eq + std::hash::Hash, Key: InternKey = usize> {
    items: Vec<T>,
    item_map: std::collections::HashMap<T, Key>,
}

impl<T, K> Default for InternTable<T, K>
where
    T: Clone + Eq + std::hash::Hash,
    K: InternKey,
{
    fn default() -> Self {
        Self {
            items: Vec::new(),
            item_map: std::collections::HashMap::new(),
        }
    }
}

impl<T: Clone + Eq + std::hash::Hash, Key: InternKey> InternTable<T, Key> {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            item_map: std::collections::HashMap::new(),
        }
    }

    pub fn intern(&mut self, item: T) -> Key {
        if let Some(&idx) = self.item_map.get(&item) {
            return idx;
        }
        let idx = Key::from(self.items.len());
        self.items.push(item.clone());
        self.item_map.insert(item, idx);
        idx
    }

    pub fn resolve(&self, idx: Key) -> Option<&T> {
        self.items.get(idx.into())
    }
}
