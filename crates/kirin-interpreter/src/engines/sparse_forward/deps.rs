//! Forward dependency bookkeeping: who reruns when something rises.
//!
//! The forward fixpoint has two kinds of dependency, and both are scheduling
//! bookkeeping rather than storage:
//!
//! - **callee summary → caller owners**, the generic
//!   [`SummaryDependencyIndex`] edge the driver consults after a summary
//!   changes (`ForwardSummaryDeps`, registered on a call, including same-key
//!   self-recursion);
//! - **context-qualified SSA fact → reader owners**, for the direct dominated
//!   cross-block uses that never travel along a block edge: a block that read a
//!   value it did not define reruns when that value rises.
//!
//! [`ForwardDeps`] holds both. Only the first is a driver-visible dependency
//! index; the second is consulted by the forward engine's own `apply_update`.

use std::collections::{HashMap, HashSet};
use std::convert::Infallible;
use std::hash::Hash;

use kirin_ir::SSAValue;

use crate::{ForwardSummaryDeps, SummaryDependencies, SummaryDependency, SummaryDependencyIndex};

use super::interp::Owner;

/// Context-qualified key for value-reader dependencies: the same [`SSAValue`]
/// under two different function contexts is two distinct facts, so readers never
/// cross-contaminate.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ValueFactKey<K> {
    pub function: K,
    pub value: SSAValue,
}

/// Both forward dependency kinds for one analysis.
pub struct ForwardDeps<K> {
    summaries: ForwardSummaryDeps<Owner<K>>,
    value_readers: HashMap<ValueFactKey<K>, HashSet<Owner<K>>>,
}

impl<K> Default for ForwardDeps<K> {
    fn default() -> Self {
        Self {
            summaries: ForwardSummaryDeps::default(),
            value_readers: HashMap::new(),
        }
    }
}

impl<K> ForwardDeps<K> {
    pub fn new() -> Self {
        Self::default()
    }
}

impl<K: Clone + Eq + Hash> ForwardDeps<K> {
    /// Record that `reader` read a value it does not define, so it must rerun
    /// when that value's fact rises.
    pub fn register_reader(&mut self, key: ValueFactKey<K>, reader: Owner<K>) {
        self.value_readers.entry(key).or_default().insert(reader);
    }

    /// The owners to reschedule now that a context-qualified value has risen.
    pub fn readers_of(&self, key: &ValueFactKey<K>) -> Vec<Owner<K>> {
        self.value_readers
            .get(key)
            .map(|readers| readers.iter().cloned().collect())
            .unwrap_or_default()
    }
}

impl<K: Clone + Eq + Hash> SummaryDependencyIndex<Owner<K>> for ForwardDeps<K> {
    type Error = Infallible;

    fn ensure_owner(&mut self, owner: &Owner<K>) -> Result<(), Self::Error> {
        self.summaries.ensure_owner(owner)
    }

    fn register(
        &mut self,
        trigger_owner: &Owner<K>,
        dependency: SummaryDependency<Owner<K>>,
    ) -> Result<(), Self::Error> {
        self.summaries.register(trigger_owner, dependency)
    }

    fn on_summary_changed<Change>(
        &mut self,
        owner: &Owner<K>,
        change: Change,
    ) -> Result<SummaryDependencies<Owner<K>>, Self::Error> {
        self.summaries.on_summary_changed(owner, change)
    }
}
