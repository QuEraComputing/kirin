//! The summary dependency index: "owner A changed → reanalyse owner B".
//!
//! [`SummaryDependencyIndex`] is the pluggable re-scheduling graph the driver
//! consults after a summary changes. [`OwnerSummaryDeps`] auto-reanalyses the
//! changed owner itself (the default self-dependent behaviour); the directional
//! [`ForwardSummaryDeps`] / [`BackwardSummaryDeps`] register only the explicit
//! edges an analysis declares (callee→caller for forward, successor→predecessor
//! for backward), so a same-owner reanalysis happens only when registered.

use std::collections::HashMap;
use std::convert::Infallible;
use std::hash::Hash;

use smallvec::{SmallVec, smallvec};

/// A single re-scheduling edge triggered when some owner's summary changes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SummaryDependency<K> {
    /// Reanalyse the given owner.
    Reanalyze(K),
}

/// The dependencies fired by one summary change.
pub type SummaryDependencies<K> = SmallVec<[SummaryDependency<K>; 2]>;

/// The pluggable "who reanalyses when A changes" graph.
pub trait SummaryDependencyIndex<K> {
    /// Error raised while mutating the index (usually [`Infallible`]).
    type Error;

    /// Register `owner` in the index (idempotent).
    fn ensure_owner(&mut self, owner: &K) -> Result<(), Self::Error>;

    /// Record that when `trigger_owner` changes, `dependency` should fire.
    fn register(
        &mut self,
        trigger_owner: &K,
        dependency: SummaryDependency<K>,
    ) -> Result<(), Self::Error>;

    /// The dependencies to schedule now that `owner`'s summary changed.
    fn on_summary_changed<Change>(
        &mut self,
        owner: &K,
        change: Change,
    ) -> Result<SummaryDependencies<K>, Self::Error>;
}

/// Dependency index whose owners are self-dependent by default: an owner that
/// changes is rescheduled unless overridden. Suitable for intra-owner iteration.
#[derive(Clone, Debug)]
pub struct OwnerSummaryDeps<K> {
    deps: HashMap<K, SummaryDependencies<K>>,
}

impl<K> Default for OwnerSummaryDeps<K> {
    fn default() -> Self {
        Self {
            deps: HashMap::new(),
        }
    }
}

impl<K> OwnerSummaryDeps<K>
where
    K: Clone + Eq + Hash,
{
    pub fn new() -> Self {
        Self::default()
    }

    fn ensure_empty_owner(&mut self, owner: &K) {
        self.deps.entry(owner.clone()).or_default();
    }

    fn ensure_dependency_owner(&mut self, owner: &K) {
        self.deps
            .entry(owner.clone())
            .or_insert_with(|| smallvec![SummaryDependency::Reanalyze(owner.clone())]);
    }

    fn register_dependency(&mut self, trigger_owner: &K, dependency: SummaryDependency<K>) {
        let deps = self
            .deps
            .entry(trigger_owner.clone())
            .or_insert_with(|| smallvec![SummaryDependency::Reanalyze(trigger_owner.clone())]);
        Self::push_unique(deps, dependency);
    }

    fn register_explicit_dependency(
        &mut self,
        trigger_owner: &K,
        dependency: SummaryDependency<K>,
    ) {
        let deps = self.deps.entry(trigger_owner.clone()).or_default();
        Self::push_unique(deps, dependency);
    }

    fn push_unique(deps: &mut SummaryDependencies<K>, dependency: SummaryDependency<K>) {
        if !deps.contains(&dependency) {
            deps.push(dependency);
        }
    }
}

impl<K> SummaryDependencyIndex<K> for OwnerSummaryDeps<K>
where
    K: Clone + Eq + Hash,
{
    type Error = Infallible;

    fn ensure_owner(&mut self, owner: &K) -> Result<(), Self::Error> {
        self.ensure_dependency_owner(owner);
        Ok(())
    }

    fn register(
        &mut self,
        trigger_owner: &K,
        dependency: SummaryDependency<K>,
    ) -> Result<(), Self::Error> {
        self.register_dependency(trigger_owner, dependency);
        Ok(())
    }

    fn on_summary_changed<Change>(
        &mut self,
        owner: &K,
        _change: Change,
    ) -> Result<SummaryDependencies<K>, Self::Error> {
        self.ensure_dependency_owner(owner);
        Ok(self.deps.get(owner).cloned().unwrap_or_default())
    }
}

/// Forward dependency index: `callee changed → reanalyse caller`. Owners are not
/// self-dependent unless a self-edge is explicitly registered (same-key
/// recursion).
#[derive(Clone, Debug)]
pub struct ForwardSummaryDeps<K> {
    deps: OwnerSummaryDeps<K>,
}

impl<K> Default for ForwardSummaryDeps<K> {
    fn default() -> Self {
        Self {
            deps: OwnerSummaryDeps::default(),
        }
    }
}

impl<K> ForwardSummaryDeps<K>
where
    K: Clone + Eq + Hash,
{
    pub fn new() -> Self {
        Self::default()
    }
}

impl<K> SummaryDependencyIndex<K> for ForwardSummaryDeps<K>
where
    K: Clone + Eq + Hash,
{
    type Error = Infallible;

    fn ensure_owner(&mut self, owner: &K) -> Result<(), Self::Error> {
        self.deps.ensure_empty_owner(owner);
        Ok(())
    }

    fn register(
        &mut self,
        trigger_owner: &K,
        dependency: SummaryDependency<K>,
    ) -> Result<(), Self::Error> {
        self.deps
            .register_explicit_dependency(trigger_owner, dependency);
        Ok(())
    }

    fn on_summary_changed<Change>(
        &mut self,
        owner: &K,
        _change: Change,
    ) -> Result<SummaryDependencies<K>, Self::Error> {
        self.deps.ensure_empty_owner(owner);
        Ok(self.deps.deps.get(owner).cloned().unwrap_or_default())
    }
}

/// Backward dependency index: `successor changed → reanalyse predecessor`. Like
/// [`ForwardSummaryDeps`], owners are self-dependent only when explicitly
/// registered.
#[derive(Clone, Debug)]
pub struct BackwardSummaryDeps<K> {
    deps: OwnerSummaryDeps<K>,
}

impl<K> Default for BackwardSummaryDeps<K> {
    fn default() -> Self {
        Self {
            deps: OwnerSummaryDeps::default(),
        }
    }
}

impl<K> BackwardSummaryDeps<K>
where
    K: Clone + Eq + Hash,
{
    pub fn new() -> Self {
        Self::default()
    }
}

impl<K> SummaryDependencyIndex<K> for BackwardSummaryDeps<K>
where
    K: Clone + Eq + Hash,
{
    type Error = Infallible;

    fn ensure_owner(&mut self, owner: &K) -> Result<(), Self::Error> {
        self.deps.ensure_empty_owner(owner);
        Ok(())
    }

    fn register(
        &mut self,
        trigger_owner: &K,
        dependency: SummaryDependency<K>,
    ) -> Result<(), Self::Error> {
        self.deps
            .register_explicit_dependency(trigger_owner, dependency);
        Ok(())
    }

    fn on_summary_changed<Change>(
        &mut self,
        owner: &K,
        _change: Change,
    ) -> Result<SummaryDependencies<K>, Self::Error> {
        self.deps.ensure_empty_owner(owner);
        Ok(self.deps.deps.get(owner).cloned().unwrap_or_default())
    }
}
