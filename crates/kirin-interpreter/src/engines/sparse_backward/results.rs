//! Owned sparse backward analysis results.

use std::collections::HashMap;
use std::hash::Hash;
use std::marker::PhantomData;

use crate::{
    AnalysisResults, Analyzed, EnvIndex, EnvStore, LinkTarget, SparseBackwardSemantic, StrongDemand,
};
use kirin_ir::{HasBottom, Lattice, Product, SSAValue};

/// Completed sparse backward facts without an engine or program reference.
/// Read through [`Analyzed`] and [`AnalysisResults`].
pub struct SparseBackwardResults<K, V, Sem: SparseBackwardSemantic = StrongDemand> {
    // Preserve rule identity without storing an engine or semantic-key value.
    pub(super) _semantics: PhantomData<fn() -> Sem>,
    pub(super) root: K,
    pub(super) env: EnvStore<K, SSAValue, V>,
    /// Reached contexts: callable and ordered entry-parameter IDs.
    pub(super) contexts: HashMap<EnvIndex, (LinkTarget, Vec<SSAValue>)>,
}

impl<K: Clone + Eq + Hash, V, Sem: SparseBackwardSemantic> SparseBackwardResults<K, V, Sem> {
    fn selected(&self, key: &K) -> Option<EnvIndex> {
        self.env
            .context_env(key)
            .filter(|env| self.contexts.contains_key(env))
    }
}

impl<K, V, Sem: SparseBackwardSemantic> AnalysisResults
    for Analyzed<SparseBackwardResults<K, V, Sem>>
where
    K: Clone + Eq + Hash,
    V: Clone + PartialEq + Lattice + HasBottom,
{
    type Context = K;
    type Anchor = SSAValue;
    type Value = V;

    fn root(&self) -> &K {
        &self.state.root
    }

    fn contexts(&self) -> impl Iterator<Item = &K> {
        self.state
            .env
            .contexts()
            .filter(|(_, env)| self.state.contexts.contains_key(env))
            .map(|(key, _)| key)
    }

    fn callable_of(&self, context: &K) -> Option<LinkTarget> {
        let env = self.state.selected(context)?;
        Some(self.state.contexts.get(&env)?.0)
    }

    fn contexts_of(&self, callable: LinkTarget) -> impl Iterator<Item = &K> {
        self.envs_of(callable)
            .filter_map(|env| self.state.env.key(env))
    }

    fn fact(&self, context: &K, value: SSAValue) -> Option<&V> {
        let env = self.state.selected(context)?;
        self.state.env.environment(env).ok()?.get(value)
    }

    fn facts(&self, context: &K) -> impl Iterator<Item = (SSAValue, &V)> {
        self.state
            .selected(context)
            .and_then(|env| self.state.env.environment(env).ok())
            .into_iter()
            .flat_map(|facts| facts.iter())
    }

    fn response(&self, context: &K) -> Option<Product<V>> {
        let env = self.state.selected(context)?;
        let params = &self.state.contexts.get(&env)?.1;
        let facts = self.state.env.environment(env).ok()?;
        Some(
            params
                .iter()
                .map(|param| facts.get(*param).cloned().unwrap_or_else(V::bottom))
                .collect(),
        )
    }
}

impl<K, V> Analyzed<SparseBackwardResults<K, V, StrongDemand>>
where
    K: Clone + Eq + Hash,
    V: Clone + PartialEq + Lattice + HasBottom,
{
    /// `true` iff `value` carries a non-bottom demand fact in `context`.
    pub fn is_demanded(&self, context: &K, value: impl Into<SSAValue>) -> bool {
        self.fact(context, value.into())
            .is_some_and(|fact| *fact != V::bottom())
    }
}
