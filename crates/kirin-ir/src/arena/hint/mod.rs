use crate::{Arena, Identifier};

mod dense;
mod sparse;

pub use dense::DenseHint;
pub use sparse::SparseHint;

pub struct HintBuilder<'a, I: Identifier, T> {
    parent: &'a Arena<I, T>,
}

impl<I: Identifier, T> Arena<I, T> {
    pub fn hint(&self) -> HintBuilder<'_, I, T> {
        HintBuilder { parent: self }
    }
}

impl<'a, I: Identifier, T> HintBuilder<'a, I, T> {
    /// create a dense hint structure for the given arena, this allows
    /// storing hints for all items in the arena efficiently, use when
    /// most items have the hints
    pub fn dense<U: Clone>(&self) -> dense::DenseHint<I, U> {
        dense::DenseHint::from_arena(self.parent)
    }

    /// create a sparse hint structure for the given arena, this allows
    /// storing hints for only some items in the arena efficiently, use when
    /// only a few items have the hints
    pub fn sparse<U>(&self) -> sparse::SparseHint<I, U> {
        sparse::SparseHint::default()
    }
}
