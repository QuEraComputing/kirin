//! The liveness lattice element: a set of live SSA values.

use std::collections::HashSet;

use kirin_ir::{Id, SSAValue};

/// A set of SSA values that are live at a given program point.
///
/// This is the lattice element of the analysis: the join is set union and the
/// bottom element is the empty set. Equality is set equality (order
/// independent), which is what the worklist solver uses to detect a fixpoint.
///
/// Identifiers do not implement [`Ord`], so the backing store is a [`HashSet`];
/// use [`LiveSet::sorted`] when a deterministic order is needed (printing,
/// test assertions).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LiveSet(HashSet<SSAValue>);

impl LiveSet {
    /// The bottom element: no values live.
    pub fn new() -> Self {
        Self(HashSet::new())
    }

    /// Insert a value; returns `true` if it was newly added.
    pub fn insert(&mut self, value: SSAValue) -> bool {
        self.0.insert(value)
    }

    /// Remove a value (a definition kills it); returns `true` if present.
    pub fn remove(&mut self, value: &SSAValue) -> bool {
        self.0.remove(value)
    }

    /// Whether `value` is live.
    pub fn contains(&self, value: &SSAValue) -> bool {
        self.0.contains(value)
    }

    /// Whether nothing is live.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Number of live values.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Iterate the live values in unspecified order (use [`LiveSet::sorted`]
    /// for a deterministic order).
    pub fn iter(&self) -> impl Iterator<Item = &SSAValue> {
        self.0.iter()
    }

    /// Join: union `other` into `self`; returns `true` if `self` changed.
    pub fn union_with(&mut self, other: &LiveSet) -> bool {
        let mut changed = false;
        for &value in &other.0 {
            changed |= self.0.insert(value);
        }
        changed
    }

    /// The live values in deterministic order, sorted by their underlying
    /// arena id.
    pub fn sorted(&self) -> Vec<SSAValue> {
        let mut values: Vec<SSAValue> = self.0.iter().copied().collect();
        values.sort_by_key(|value| Id::from(*value).raw());
        values
    }
}

impl FromIterator<SSAValue> for LiveSet {
    fn from_iter<I: IntoIterator<Item = SSAValue>>(iter: I) -> Self {
        Self(iter.into_iter().collect())
    }
}
