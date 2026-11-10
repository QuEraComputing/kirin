//! Shared anchor-keyed storage for interpreter values and analysis facts.
//!
//! [`FactStore<A, F>`] holds one payload `F` per [`LatticeAnchor`] `A`.
//! [`EnvStore`](crate::EnvStore) allocates one of these per environment, addressed by
//! analysis context, for concrete and forward abstract interpretation. Backward
//! analyses use the map directly with SSA or program-point anchors, qualified by
//! scope where needed. Engines decide what absence means and when facts must
//! join.

use rustc_hash::FxHashMap;

use super::anchor::LatticeAnchor;

/// One interpreter value or analysis fact per [`LatticeAnchor`].
///
/// Missing anchors return `None`. Concrete engines report unbound values;
/// abstract engines can interpret absence as bottom. Assignment never joins
/// implicitly.
///
/// Anchors are small integer ids or tuples of them, so the map uses Fx hashing
/// instead of the std default (SipHash).
#[derive(Clone, Debug)]
pub struct FactStore<A, F>
where
    A: LatticeAnchor,
{
    facts: FxHashMap<A, F>,
}

impl<A: LatticeAnchor, F> Default for FactStore<A, F> {
    fn default() -> Self {
        Self::new()
    }
}

impl<A: LatticeAnchor, F> FactStore<A, F> {
    pub fn new() -> Self {
        Self {
            facts: FxHashMap::default(),
        }
    }

    /// Read the fact stored at `anchor`, or `None` if absent.
    pub fn get(&self, anchor: A) -> Option<&F> {
        self.facts.get(&anchor)
    }

    /// The fact stored at `anchor`, for an in-place update, or `None` if absent.
    pub fn get_mut(&mut self, anchor: A) -> Option<&mut F> {
        self.facts.get_mut(&anchor)
    }

    /// Overwrite the fact at `anchor`.
    pub fn set(&mut self, anchor: A, fact: F) {
        self.facts.insert(anchor, fact);
    }

    /// `true` if a fact is explicitly stored at `anchor`, including bottom.
    pub fn contains(&self, anchor: A) -> bool {
        self.facts.contains_key(&anchor)
    }

    /// Number of explicitly stored facts.
    pub fn len(&self) -> usize {
        self.facts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.facts.is_empty()
    }

    /// Iterate stored `(anchor, fact)` pairs (anchors cloned; order unspecified).
    pub fn iter(&self) -> impl Iterator<Item = (A, &F)>
    where
        A: Clone,
    {
        self.facts
            .iter()
            .map(|(anchor, fact)| (anchor.clone(), fact))
    }
}

#[cfg(test)]
mod tests {
    use crate::ProgramPoint;
    use kirin_ir::{Block, Id, SSAValue, Statement, TestSSAValue};

    use super::*;

    fn ssa(n: usize) -> SSAValue {
        SSAValue::from(TestSSAValue(n))
    }

    #[test]
    fn fact_store_get_set() {
        let mut store: FactStore<SSAValue, u32> = FactStore::new();
        assert!(store.is_empty());
        assert_eq!(store.get(ssa(1)), None);

        store.set(ssa(1), 3);
        assert!(store.contains(ssa(1)));
        assert_eq!(store.get(ssa(1)), Some(&3));
        assert_eq!(store.len(), 1);

        // Assignment replaces the stored fact.
        store.set(ssa(1), 5);
        store.set(ssa(2), 1);
        assert_eq!(store.get(ssa(1)), Some(&5));

        let mut pairs: Vec<(SSAValue, u32)> =
            store.iter().map(|(anchor, fact)| (anchor, *fact)).collect();
        pairs.sort_by_key(|(anchor, _)| Id::from(*anchor).raw());
        assert_eq!(pairs, vec![(ssa(1), 5), (ssa(2), 1)]);
    }

    #[test]
    fn dense_facts_keep_block_and_statement_boundaries_distinct() {
        let block = Block::from(Id::from(ssa(0)));
        let other = Block::from(Id::from(ssa(1)));
        let statement = Statement::from(Id::from(ssa(3)));

        let mut store: FactStore<ProgramPoint, &'static str> = FactStore::new();
        store.set(ProgramPoint::BlockEntry(block), "in");
        store.set(ProgramPoint::BlockExit(block), "out");
        store.set(ProgramPoint::Before(statement), "before");
        store.set(ProgramPoint::After(statement), "after");

        assert_eq!(store.get(ProgramPoint::BlockEntry(block)), Some(&"in"));
        assert_eq!(store.get(ProgramPoint::BlockExit(block)), Some(&"out"));
        assert_eq!(store.get(ProgramPoint::BlockEntry(other)), None);
        assert_eq!(store.get(ProgramPoint::Before(statement)), Some(&"before"));
        assert_eq!(store.get(ProgramPoint::After(statement)), Some(&"after"));
        assert_eq!(store.len(), 4);
    }
}
