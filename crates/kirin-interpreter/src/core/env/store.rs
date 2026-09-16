use std::collections::HashMap;
use std::hash::Hash;

use crate::{FactStore, InterpreterError, LatticeAnchor};

/// A handle to one allocated environment. The owning engine controls its
/// lifetime; the handle stays invalid once freed, because indices are never
/// reused.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EnvIndex(usize);

impl EnvIndex {
    pub(crate) fn new(index: usize) -> Self {
        Self(index)
    }

    pub fn raw(self) -> usize {
        self.0
    }
}

/// One allocated environment: its facts, plus the context key it is registered
/// under, if any. Keeping the key next to the facts is what lets
/// [`EnvStore::free`] drop a context association in constant time without
/// scanning either the directory or the facts.
#[derive(Clone, Debug)]
struct Environment<K, A, V>
where
    A: LatticeAnchor,
{
    key: Option<K>,
    facts: FactStore<A, V>,
}

/// Environments plus the directory that addresses them by context key.
///
/// One container owns both halves of environment identity: `K -> EnvIndex`
/// (which analysis context an environment belongs to) and
/// `EnvIndex -> FactStore<A, V>` (the facts it holds). Fact maps stay separate
/// per environment, so equal anchors under different contexts never collide.
///
/// The container is deliberately ignorant of what it stores. It knows nothing
/// about bottom values, widening, dependencies, or scheduling, and an absent
/// anchor is reported as absent rather than interpreted: what absence *means*
/// is the engine's decision, made in [`Env`](crate::Env).
/// [`write`](Self::write) assigns and never joins.
///
/// The *analysis* chooses context identity — a key `K` derived from a resolved
/// call target and its abstract arguments — and this container only maps that
/// identity to a live environment. Concrete execution has no context identity
/// at all: it allocates with [`alloc`](Self::alloc) and uses an uninhabited key
/// type, so two calls can never accidentally share an environment.
#[derive(Clone, Debug)]
pub struct EnvStore<K, A, V>
where
    A: LatticeAnchor,
{
    context_indices: HashMap<K, EnvIndex>,
    environments: Vec<Option<Environment<K, A, V>>>,
}

impl<K, A, V> Default for EnvStore<K, A, V>
where
    A: LatticeAnchor,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<K, A, V> EnvStore<K, A, V>
where
    A: LatticeAnchor,
{
    pub fn new() -> Self {
        Self {
            context_indices: HashMap::new(),
            environments: Vec::new(),
        }
    }

    /// Allocate a fresh environment with no context association.
    ///
    /// Every call returns a distinct handle to a distinct, empty environment.
    pub fn alloc(&mut self) -> EnvIndex {
        self.alloc_with(None)
    }

    /// Inspect the facts of a live environment.
    pub fn environment(&self, index: EnvIndex) -> Result<&FactStore<A, V>, InterpreterError> {
        self.environments
            .get(index.raw())
            .and_then(Option::as_ref)
            .map(|environment| &environment.facts)
            .ok_or(InterpreterError::InvalidEnvIndex(index))
    }

    /// Read the value stored at `anchor`, or `None` when the anchor holds
    /// nothing.
    ///
    /// The only error is an invalid `index`, which is what keeps a dead
    /// environment distinguishable from an absent anchor.
    pub fn read(&self, index: EnvIndex, anchor: A) -> Result<Option<V>, InterpreterError>
    where
        V: Clone,
    {
        Ok(self.environment(index)?.get(anchor).cloned())
    }

    /// Assign `value` at `anchor`, replacing whatever was stored there.
    pub fn write(&mut self, index: EnvIndex, anchor: A, value: V) -> Result<(), InterpreterError> {
        self.environment_mut(index)?.set(anchor, value);
        Ok(())
    }

    fn alloc_with(&mut self, key: Option<K>) -> EnvIndex {
        let index = EnvIndex::new(self.environments.len());
        self.environments.push(Some(Environment {
            key,
            facts: FactStore::new(),
        }));
        index
    }

    fn environment_mut(
        &mut self,
        index: EnvIndex,
    ) -> Result<&mut FactStore<A, V>, InterpreterError> {
        self.environments
            .get_mut(index.raw())
            .and_then(Option::as_mut)
            .map(|environment| &mut environment.facts)
            .ok_or(InterpreterError::InvalidEnvIndex(index))
    }
}

impl<K, A, V> EnvStore<K, A, V>
where
    K: Clone + Eq + Hash,
    A: LatticeAnchor,
{
    /// The environment registered under `key`, without allocating one.
    ///
    /// Every directory entry addresses a live environment: [`free`](Self::free)
    /// drops the association along with the environment.
    pub fn context_env(&self, key: &K) -> Option<EnvIndex> {
        self.context_indices.get(key).copied()
    }

    /// The environment registered under `key`, allocating and registering one on
    /// first use.
    ///
    /// Equal keys share one environment for as long as it stays live; distinct
    /// keys are isolated.
    pub fn get_or_allocate(&mut self, key: K) -> EnvIndex {
        if let Some(index) = self.context_env(&key) {
            return index;
        }
        let index = self.alloc_with(Some(key.clone()));
        self.context_indices.insert(key, index);
        index
    }

    /// Retire an environment and drop its context association, if it had one.
    ///
    /// The handle stays invalid afterwards, and a later
    /// [`get_or_allocate`](Self::get_or_allocate) of the same key allocates a
    /// fresh environment rather than resurrecting this one.
    pub fn free(&mut self, index: EnvIndex) -> Result<(), InterpreterError> {
        let environment = self
            .environments
            .get_mut(index.raw())
            .and_then(Option::take)
            .ok_or(InterpreterError::InvalidEnvIndex(index))?;
        if let Some(key) = environment.key {
            self.context_indices.remove(&key);
        }
        Ok(())
    }
}
