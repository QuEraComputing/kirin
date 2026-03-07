pub trait InternKey:
    From<usize> + Into<usize> + Clone + Copy + PartialEq + Eq + std::hash::Hash
{
}

impl InternKey for usize {}

#[derive(Clone, Debug)]
pub struct InternTable<T: Clone + Eq + std::hash::Hash, Key: InternKey = usize> {
    items: Vec<T>,
    item_map: rustc_hash::FxHashMap<T, Key>,
}

impl<T, K> Default for InternTable<T, K>
where
    T: Clone + Eq + std::hash::Hash,
    K: InternKey,
{
    fn default() -> Self {
        Self {
            items: Vec::new(),
            item_map: rustc_hash::FxHashMap::default(),
        }
    }
}

impl<T: Clone + Eq + std::hash::Hash, Key: InternKey> InternTable<T, Key> {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            item_map: rustc_hash::FxHashMap::default(),
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

    pub fn resolve(&self, idx: impl Into<Key>) -> Option<&T> {
        let idx: usize = idx.into().into();
        self.items.get(idx)
    }

    /// Look up a previously interned item without inserting.
    ///
    /// Returns `None` if the item has never been interned.
    ///
    /// Accepts any type that `T` borrows as (e.g., `&str` for `String` keys),
    /// avoiding unnecessary allocation on lookup.
    pub fn lookup<Q>(&self, item: &Q) -> Option<Key>
    where
        T: std::borrow::Borrow<Q>,
        Q: Eq + std::hash::Hash + ?Sized,
    {
        self.item_map.get(item).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intern_table_idempotent() {
        let mut table: InternTable<String> = InternTable::new();
        let k1 = table.intern("hello".to_string());
        let k2 = table.intern("hello".to_string());
        assert_eq!(
            k1, k2,
            "interning the same string twice must return the same key"
        );
    }

    #[test]
    fn intern_resolve_roundtrip() {
        let mut table: InternTable<String> = InternTable::new();
        let key = table.intern("world".to_string());
        let resolved = table.resolve(key);
        assert_eq!(resolved, Some(&"world".to_string()));
    }

    #[test]
    fn resolve_out_of_bounds_returns_none() {
        let table: InternTable<String> = InternTable::new();
        assert_eq!(table.resolve(42usize), None);
    }

    #[test]
    fn lookup_returns_none_for_missing() {
        let table: InternTable<String> = InternTable::new();
        assert_eq!(table.lookup("missing"), None);
    }

    #[test]
    fn lookup_returns_key_for_interned() {
        let mut table: InternTable<String> = InternTable::new();
        let key = table.intern("found".to_string());
        assert_eq!(table.lookup("found"), Some(key));
    }

    #[test]
    fn distinct_strings_get_distinct_keys() {
        let mut table: InternTable<String> = InternTable::new();
        let k1 = table.intern("alpha".to_string());
        let k2 = table.intern("beta".to_string());
        assert_ne!(k1, k2);
    }
}
