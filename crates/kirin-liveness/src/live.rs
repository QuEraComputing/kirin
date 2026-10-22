//! The live-set point state of classic liveness.

use std::collections::HashSet;

use kirin_interpreter::PointFacts;
use kirin_ir::{HasBottom, Lattice, SSAValue};

/// A set of live SSA values.
///
/// Backed by a `HashSet`; [`sorted`](Self::sorted) returns a deterministic order
/// keyed by the SSA value's raw id (a stable key that needs no core `Ord` impl).
#[derive(Clone, Default, PartialEq, Eq)]
pub struct LiveSet {
    values: HashSet<SSAValue>,
}

impl LiveSet {
    pub fn new() -> Self {
        Self {
            values: HashSet::new(),
        }
    }

    /// Insert `value`, returning `true` if it was newly added.
    pub fn insert(&mut self, value: SSAValue) -> bool {
        self.values.insert(value)
    }

    /// Remove `value`, returning `true` if it was present.
    pub fn remove(&mut self, value: SSAValue) -> bool {
        self.values.remove(&value)
    }

    pub fn contains(&self, value: SSAValue) -> bool {
        self.values.contains(&value)
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = SSAValue> + '_ {
        self.values.iter().copied()
    }

    /// Union `other` into `self`, returning `true` if anything was added.
    pub fn union_with(&mut self, other: &LiveSet) -> bool {
        let mut changed = false;
        for value in other.values.iter().copied() {
            changed |= self.values.insert(value);
        }
        changed
    }

    /// The live values in a deterministic order (by raw SSA id).
    pub fn sorted(&self) -> Vec<SSAValue> {
        let mut out: Vec<SSAValue> = self.values.iter().copied().collect();
        out.sort_by_key(|v| kirin_ir::Id::from(*v).raw());
        out
    }
}

impl Lattice for LiveSet {
    /// Set union: live on *any* path.
    fn join(&self, other: &Self) -> Self {
        let mut joined = self.clone();
        joined.union_with(other);
        joined
    }

    /// Set intersection: live on *all* paths.
    fn meet(&self, other: &Self) -> Self {
        Self {
            values: self.values.intersection(&other.values).copied().collect(),
        }
    }

    fn is_subseteq(&self, other: &Self) -> bool {
        self.values.is_subset(&other.values)
    }
}

impl HasBottom for LiveSet {
    fn bottom() -> Self {
        Self::new()
    }
}

/// The dense backward point-state contract: gen/kill mutate the set.
impl PointFacts for LiveSet {
    fn insert(&mut self, value: SSAValue) -> bool {
        LiveSet::insert(self, value)
    }

    fn remove(&mut self, value: SSAValue) -> bool {
        LiveSet::remove(self, value)
    }

    fn contains(&self, value: SSAValue) -> bool {
        LiveSet::contains(self, value)
    }

    fn values(&self) -> Vec<SSAValue> {
        self.iter().collect()
    }
}

impl FromIterator<SSAValue> for LiveSet {
    fn from_iter<T: IntoIterator<Item = SSAValue>>(iter: T) -> Self {
        Self {
            values: iter.into_iter().collect(),
        }
    }
}

impl std::fmt::Debug for LiveSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_set().entries(self.sorted()).finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kirin_ir::TestSSAValue;

    fn ssa(n: usize) -> SSAValue {
        SSAValue::from(TestSSAValue(n))
    }

    #[test]
    fn live_set_union_reports_change() {
        let mut a = LiveSet::new();
        assert!(a.insert(ssa(1)));
        assert!(!a.insert(ssa(1)));

        let b: LiveSet = [ssa(1), ssa(2)].into_iter().collect();
        assert!(a.union_with(&b)); // ssa(2) added
        assert!(!a.union_with(&b)); // already a superset
        assert_eq!(a.len(), 2);
    }

    #[test]
    fn live_set_sorted_is_deterministic() {
        let set: LiveSet = [ssa(3), ssa(1), ssa(2)].into_iter().collect();
        let sorted = set.sorted();
        assert_eq!(sorted, vec![ssa(1), ssa(2), ssa(3)]);
    }
}
