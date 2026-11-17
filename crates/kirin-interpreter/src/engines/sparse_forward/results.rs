//! Owned sparse forward analysis results.

use std::collections::HashMap;
use std::hash::Hash;
use std::marker::PhantomData;

use crate::{
    AnalysisResults, Analyzed, EnvIndex, EnvStore, ForwardEval, LinkTarget, SparseForwardSemantic,
};
use kirin_ir::{Lattice, Product, SSAValue};

/// Completed sparse forward facts without an engine or program reference.
/// Read through [`Analyzed`] and [`AnalysisResults`].
pub struct SparseForwardResults<K, V, Sem: SparseForwardSemantic = ForwardEval> {
    // Preserve rule identity without storing an engine or semantic-key value.
    pub(super) _semantics: PhantomData<fn() -> Sem>,
    pub(super) root: K,
    pub(super) env: EnvStore<K, SSAValue, V>,
    /// Reached contexts: callable and return summary.
    pub(super) contexts: HashMap<EnvIndex, (LinkTarget, Option<Product<V>>)>,
}

impl<K: Clone + Eq + Hash, V, Sem: SparseForwardSemantic> SparseForwardResults<K, V, Sem> {
    fn selected(&self, key: &K) -> Option<EnvIndex> {
        self.env
            .context_env(key)
            .filter(|env| self.contexts.contains_key(env))
    }
}

impl<K, V, Sem: SparseForwardSemantic> AnalysisResults for Analyzed<SparseForwardResults<K, V, Sem>>
where
    K: Clone + Eq + Hash,
    V: Clone + Lattice,
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
        self.state.contexts.get(&env)?.1.clone()
    }
}
