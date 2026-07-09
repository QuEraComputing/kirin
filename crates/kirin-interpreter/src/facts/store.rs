//! Anchor-keyed dataflow fact stores.
//!
//! [`FactStore<A, F>`] is the reusable fact container: one fact `F` per
//! [`LatticeAnchor`] `A`, with absent anchors carrying the analysis's bottom
//! fact. It is deliberately separate from the operational activation store
//! ([`EnvIndex`](crate::EnvIndex) / [`EnvStackStore`](crate::EnvStackStore)):
//! that is the CESK-style runtime/abstract-execution environment handle, while
//! this is where analyses keep dataflow facts. The familiar stores are
//! instantiations picked by the analysis's anchor: sparse analyses anchor
//! facts to SSA values ([`SparseStore`], scope-qualified as
//! [`ScopedSparseStore`]), dense analyses to program points
//! ([`DensePointStore`]) or block boundaries ([`DenseBlockStore`]).

use std::collections::HashMap;

use kirin_ir::SSAValue;

use super::anchor::{Change, DenseAnchor, LatticeAnchor, ProgramPoint, Scoped};

/// One dataflow fact per lattice anchor.
///
/// Anchors absent from the store carry the analysis's bottom fact.
#[derive(Clone, Debug)]
pub struct FactStore<A, F>
where
    A: LatticeAnchor,
{
    facts: HashMap<A, F>,
}

impl<A: LatticeAnchor, F> Default for FactStore<A, F> {
    fn default() -> Self {
        Self::new()
    }
}

impl<A: LatticeAnchor, F> FactStore<A, F> {
    pub fn new() -> Self {
        Self {
            facts: HashMap::new(),
        }
    }

    /// Read the fact stored at `anchor`, or `None` if it carries bottom.
    pub fn get(&self, anchor: A) -> Option<&F> {
        self.facts.get(&anchor)
    }

    /// Overwrite the fact at `anchor`.
    pub fn set(&mut self, anchor: A, fact: F) {
        self.facts.insert(anchor, fact);
    }

    /// `true` if a (non-bottom) fact is stored at `anchor`.
    pub fn contains(&self, anchor: A) -> bool {
        self.facts.contains_key(&anchor)
    }

    /// Number of anchors carrying a non-bottom fact.
    pub fn len(&self) -> usize {
        self.facts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.facts.is_empty()
    }

    /// Iterate `(anchor, fact)` pairs for anchors carrying a non-bottom fact
    /// (anchors cloned; order unspecified).
    pub fn iter(&self) -> impl Iterator<Item = (A, &F)> {
        self.facts
            .iter()
            .map(|(anchor, fact)| (anchor.clone(), fact))
    }
}

impl<A: LatticeAnchor, F: PartialEq> FactStore<A, F> {
    /// Join `incoming` into the fact at `anchor` using `merge`, reporting
    /// whether the stored fact changed. `bottom` supplies the implicit fact
    /// for an absent anchor.
    pub fn join_with(
        &mut self,
        anchor: A,
        incoming: &F,
        bottom: &F,
        merge: impl Fn(&F, &F) -> F,
    ) -> Change {
        let old = self.facts.get(&anchor).unwrap_or(bottom);
        let merged = merge(old, incoming);
        if &merged == old {
            Change::Unchanged
        } else {
            self.facts.insert(anchor, merged);
            Change::Changed
        }
    }
}

/// A sparse store keyed by SSA values, for sparse-shaped analyses
/// ([`SparseForwardShape`](crate::SparseForwardShape) /
/// [`SparseBackwardShape`](crate::SparseBackwardShape)).
pub type SparseStore<F> = FactStore<SSAValue, F>;

/// A sparse store whose SSA-value anchors are scope-qualified: the same value
/// under two scopes is two distinct facts.
pub type ScopedSparseStore<K, F> = FactStore<Scoped<K, SSAValue>, F>;

/// A dense store keyed by program points, for analyses that need per-point
/// state as a queryable fact (e.g. reconstructed per-statement live sets).
pub type DensePointStore<F> = FactStore<ProgramPoint, F>;

/// A dense store keyed by block boundaries (entry/exit), for dense analyses.
///
/// This is the rustc-style default: store block-boundary states and
/// reconstruct statement-local states on demand rather than persisting every
/// before/after state. A thin convenience wrapper over
/// `FactStore<DenseAnchor, F>` using the
/// [`BlockEntry`](DenseAnchor::BlockEntry)/[`BlockExit`](DenseAnchor::BlockExit)
/// anchors.
#[derive(Clone, Debug)]
pub struct DenseBlockStore<F> {
    facts: FactStore<DenseAnchor, F>,
}

impl<F> Default for DenseBlockStore<F> {
    fn default() -> Self {
        Self::new()
    }
}

impl<F> DenseBlockStore<F> {
    pub fn new() -> Self {
        Self {
            facts: FactStore::new(),
        }
    }

    pub fn entry(&self, block: kirin_ir::Block) -> Option<&F> {
        self.facts.get(DenseAnchor::BlockEntry(block))
    }

    pub fn exit(&self, block: kirin_ir::Block) -> Option<&F> {
        self.facts.get(DenseAnchor::BlockExit(block))
    }

    pub fn set_entry(&mut self, block: kirin_ir::Block, fact: F) {
        self.facts.set(DenseAnchor::BlockEntry(block), fact);
    }

    pub fn set_exit(&mut self, block: kirin_ir::Block, fact: F) {
        self.facts.set(DenseAnchor::BlockExit(block), fact);
    }

    /// Iterate `(block, entry fact)` pairs (order unspecified).
    pub fn entries(&self) -> impl Iterator<Item = (kirin_ir::Block, &F)> {
        self.facts.iter().filter_map(|(anchor, fact)| match anchor {
            DenseAnchor::BlockEntry(block) => Some((block, fact)),
            _ => None,
        })
    }

    /// Iterate `(block, exit fact)` pairs (order unspecified).
    pub fn exits(&self) -> impl Iterator<Item = (kirin_ir::Block, &F)> {
        self.facts.iter().filter_map(|(anchor, fact)| match anchor {
            DenseAnchor::BlockExit(block) => Some((block, fact)),
            _ => None,
        })
    }
}

#[cfg(test)]
mod tests {
    use kirin_ir::{Block, CompileStage, Id, Region, Statement, TestSSAValue};

    use super::*;

    fn ssa(n: usize) -> SSAValue {
        SSAValue::from(TestSSAValue(n))
    }

    #[test]
    fn fact_store_get_set_join() {
        let mut store: SparseStore<u32> = FactStore::new();
        assert!(store.is_empty());
        assert_eq!(store.get(ssa(1)), None);

        store.set(ssa(1), 3);
        assert!(store.contains(ssa(1)));
        assert_eq!(store.get(ssa(1)), Some(&3));
        assert_eq!(store.len(), 1);

        // Join with max: rising changes, non-rising doesn't.
        let max = |a: &u32, b: &u32| *a.max(b);
        assert_eq!(store.join_with(ssa(1), &5, &0, max), Change::Changed);
        assert_eq!(store.join_with(ssa(1), &4, &0, max), Change::Unchanged);
        assert_eq!(store.get(ssa(1)), Some(&5));
        // An absent anchor joins from the supplied bottom.
        assert_eq!(store.join_with(ssa(2), &1, &0, max), Change::Changed);

        let mut pairs: Vec<(SSAValue, u32)> =
            store.iter().map(|(anchor, fact)| (anchor, *fact)).collect();
        pairs.sort_by_key(|(anchor, _)| Id::from(*anchor).raw());
        assert_eq!(pairs, vec![(ssa(1), 5), (ssa(2), 1)]);
    }

    #[test]
    fn scoped_anchors_keep_scopes_distinct() {
        type Scope = (CompileStage, Region);
        let scope_a: Scope = (
            CompileStage::from(Id::from(ssa(100))),
            Region::from(Id::from(ssa(200))),
        );
        let scope_b: Scope = (
            CompileStage::from(Id::from(ssa(101))),
            Region::from(Id::from(ssa(200))),
        );

        let mut store: ScopedSparseStore<Scope, &'static str> = FactStore::new();
        // The SAME SSA value under two scopes is two distinct facts.
        store.set(Scoped::new(scope_a, ssa(7)), "a");
        store.set(Scoped::new(scope_b, ssa(7)), "b");

        assert_eq!(store.len(), 2);
        assert_eq!(store.get(Scoped::new(scope_a, ssa(7))), Some(&"a"));
        assert_eq!(store.get(Scoped::new(scope_b, ssa(7))), Some(&"b"));
        assert!(!store.contains(Scoped::new(scope_a, ssa(8))));
    }

    #[test]
    fn dense_block_store_maps_entry_exit_through_dense_anchor() {
        let block = Block::from(Id::from(ssa(0)));
        let other = Block::from(Id::from(ssa(1)));

        let mut store: DenseBlockStore<&'static str> = DenseBlockStore::new();
        store.set_entry(block, "in");
        store.set_exit(block, "out");

        // Entry and exit of the same block are distinct anchors.
        assert_eq!(store.entry(block), Some(&"in"));
        assert_eq!(store.exit(block), Some(&"out"));
        assert_eq!(store.entry(other), None);

        let entries: Vec<_> = store.entries().collect();
        let exits: Vec<_> = store.exits().collect();
        assert_eq!(entries, vec![(block, &"in")]);
        assert_eq!(exits, vec![(block, &"out")]);
    }

    #[test]
    fn dense_point_store_keeps_before_and_after_distinct() {
        let statement = Statement::from(Id::from(ssa(3)));

        let mut store: DensePointStore<&'static str> = FactStore::new();
        store.set(ProgramPoint::Before(statement), "before");
        store.set(ProgramPoint::After(statement), "after");

        assert_eq!(store.get(ProgramPoint::Before(statement)), Some(&"before"));
        assert_eq!(store.get(ProgramPoint::After(statement)), Some(&"after"));
        assert_eq!(store.len(), 2);
    }
}
