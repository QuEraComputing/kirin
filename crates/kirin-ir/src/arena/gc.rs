use super::data::Arena;
use super::id::{Id, Identifier};

/// IdMap from old -> new
///
/// Not yet used externally — GC infrastructure is retained for future use.
#[allow(dead_code)]
pub struct IdMap<I: Identifier>(Vec<Option<I>>);

#[allow(dead_code)]
impl<I: Identifier> IdMap<I> {
    pub fn get(&self, old: I) -> Option<I> {
        self.0.get(old.into().raw()).copied().flatten()
    }
}

impl<I: Identifier, T> Arena<I, T> {
    /// Compact the arena by removing deleted items and remapping IDs.
    ///
    /// Returns an [`IdMap`] that maps old IDs to their new positions.
    ///
    /// # Safety Hazard
    ///
    /// After calling `gc()`, **all previously obtained IDs become stale**.
    /// Any `I` values stored externally (in other arenas, caches, IR nodes, etc.)
    /// must be remapped through the returned [`IdMap`] before use. Accessing the
    /// arena with a stale ID will return incorrect data or panic.
    ///
    /// Callers are responsible for updating all external references. There is
    /// currently no runtime detection of stale IDs.
    #[must_use]
    #[allow(dead_code)]
    pub(crate) fn gc(&mut self) -> IdMap<I> {
        let mut counter = 0;
        let raw = self
            .items
            .iter()
            .map(|item| {
                if item.deleted() {
                    None
                } else {
                    counter += 1;
                    Some(I::from(Id(counter - 1)))
                }
            })
            .collect::<Vec<_>>();
        self.items.retain(|item| !item.deleted());
        IdMap(raw)
    }
}
