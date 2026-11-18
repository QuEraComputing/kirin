//! Reading converged analysis facts: [`Analyzed`] is what an abstract
//! engine's `analyze` returns, and [`AnalysisResults`] is the one read-only
//! view all three engines share.

use std::collections::{HashMap, HashSet};
use std::hash::Hash;
use std::marker::PhantomData;

use kirin_ir::{HasBottom, Lattice, Product, SSAValue};

use crate::{EnvIndex, EnvStore, FactStore, LatticeAnchor, LinkTarget, StrongDemand};

/// The owned result of one converged analysis: facts of context key `K`
/// attached at anchor `A` with lattice value `V`, computed under semantic key
/// `Sem`. It holds no engine and no borrow of the program.
///
/// Only `analyze` builds one, so it always has a root, and it cannot run
/// again: analyzing another root takes another engine. Program edits do not
/// update its facts. Read it through [`AnalysisResults`].
pub struct Analyzed<K, A: LatticeAnchor, V, Sem> {
    root: K,
    /// The converged fact store, moved out of the engine.
    env: EnvStore<K, A, V>,
    /// Each reached context's callable and response.
    contexts: HashMap<EnvIndex, (LinkTarget, Option<Product<V>>)>,
    /// The environments of each reached callable's contexts.
    by_callable: HashMap<LinkTarget, Vec<EnvIndex>>,
    /// Keeps two semantic keys that share an engine distinct types.
    _semantics: PhantomData<fn() -> Sem>,
}

impl<K, A: LatticeAnchor, V, Sem> Analyzed<K, A, V, Sem> {
    /// Take ownership of a converged engine's fact store, with each reached
    /// context's environment, callable and response.
    pub(crate) fn new(
        root: K,
        env: EnvStore<K, A, V>,
        contexts: impl IntoIterator<Item = (EnvIndex, LinkTarget, Option<Product<V>>)>,
    ) -> Self {
        let mut by_callable: HashMap<LinkTarget, Vec<EnvIndex>> = HashMap::new();
        let contexts = contexts
            .into_iter()
            .map(|(index, callable, response)| {
                by_callable.entry(callable).or_default().push(index);
                (index, (callable, response))
            })
            .collect();
        Self {
            root,
            env,
            contexts,
            by_callable,
            _semantics: PhantomData,
        }
    }
}

impl<K: Clone + Eq + Hash, A: LatticeAnchor, V, Sem> Analyzed<K, A, V, Sem> {
    /// The environment of `key`, if the analysis reached it.
    fn reached(&self, key: &K) -> Option<EnvIndex> {
        self.env
            .context_env(key)
            .filter(|index| self.contexts.contains_key(index))
    }
}

/// The read-only view of a converged analysis.
///
/// A *context* is one analyzed copy of a callable under specific boundary
/// facts. Facts are read at two levels: per context, and per callable (joined
/// over the callable's contexts). The *root* is the context `analyze` was
/// asked for; it is read like any other context.
///
/// Per-callable answers join the contexts **reached from the root**. They say
/// nothing about callers the analysis never visited.
pub trait AnalysisResults {
    /// The identity of one analyzed copy of a callable.
    type Context;
    /// Where a fact attaches: an SSA value or a program point.
    type Anchor: LatticeAnchor;
    type Value: Lattice + Clone;

    /// The context `analyze` was asked for.
    fn root(&self) -> &Self::Context;

    /// Every context reached from the root, the root included, in no
    /// particular order.
    fn contexts(&self) -> impl Iterator<Item = &Self::Context>;

    /// Which callable `context` is a copy of; `None` if the analysis did not
    /// reach `context`.
    fn callable_of(&self, context: &Self::Context) -> Option<LinkTarget>;

    /// The fact at `anchor` in `context`. `None` means absent, which is
    /// bottom.
    fn fact(&self, context: &Self::Context, anchor: Self::Anchor) -> Option<&Self::Value>;

    /// Every stored fact of `context`.
    fn facts(&self, context: &Self::Context) -> impl Iterator<Item = (Self::Anchor, &Self::Value)>;

    /// What `context` answers its caller, one fact per slot. `None` means it
    /// has no answer, which differs from an empty one.
    fn response(&self, context: &Self::Context) -> Option<Product<Self::Value>>;

    /// Every callable reached, each once, in no particular order.
    fn callables(&self) -> impl Iterator<Item = LinkTarget> {
        let mut seen = HashSet::new();
        self.contexts()
            .filter_map(|context| self.callable_of(context))
            .filter(move |callable| seen.insert(*callable))
    }

    /// The contexts that analyze `callable`.
    fn contexts_of(&self, callable: LinkTarget) -> impl Iterator<Item = &Self::Context> {
        self.contexts()
            .filter(move |context| self.callable_of(context) == Some(callable))
    }

    /// The join of the fact at `anchor` over `callable`'s contexts. `None`
    /// means absent in all of them.
    fn joined_fact(&self, callable: LinkTarget, anchor: Self::Anchor) -> Option<Self::Value> {
        self.contexts_of(callable)
            .filter_map(|context| self.fact(context, anchor.clone()))
            .cloned()
            .reduce(|joined, fact| joined.join(&fact))
    }

    /// The join of every stored fact over `callable`'s contexts.
    fn joined_facts(&self, callable: LinkTarget) -> FactStore<Self::Anchor, Self::Value> {
        let mut joined: FactStore<Self::Anchor, Self::Value> = FactStore::new();
        for context in self.contexts_of(callable) {
            for (anchor, fact) in self.facts(context) {
                match joined.get_mut(anchor.clone()) {
                    Some(current) => {
                        current.join_assign(fact);
                    }
                    None => joined.set(anchor, fact.clone()),
                }
            }
        }
        joined
    }

    /// The slot-wise join of [`response`](Self::response) over `callable`'s
    /// contexts that answered. `None` if none answered, or if two answers
    /// disagree on arity.
    fn joined_response(&self, callable: LinkTarget) -> Option<Product<Self::Value>> {
        let mut responses = self
            .contexts_of(callable)
            .filter_map(|context| self.response(context));
        let mut joined = responses.next()?;
        for response in responses {
            if response.len() != joined.len() {
                return None;
            }
            for (slot, fact) in joined.iter_mut().zip(response.iter()) {
                slot.join_assign(fact);
            }
        }
        Some(joined)
    }
}

impl<K, A, V, Sem> AnalysisResults for Analyzed<K, A, V, Sem>
where
    K: Clone + Eq + Hash,
    A: LatticeAnchor,
    V: Clone + Lattice,
{
    type Context = K;
    type Anchor = A;
    type Value = V;

    fn root(&self) -> &K {
        &self.root
    }

    fn contexts(&self) -> impl Iterator<Item = &K> {
        self.env
            .contexts()
            .filter(|(_, index)| self.contexts.contains_key(index))
            .map(|(key, _)| key)
    }

    fn callable_of(&self, context: &K) -> Option<LinkTarget> {
        Some(self.contexts.get(&self.reached(context)?)?.0)
    }

    fn contexts_of(&self, callable: LinkTarget) -> impl Iterator<Item = &K> {
        self.by_callable
            .get(&callable)
            .into_iter()
            .flatten()
            .filter_map(|&index| self.env.key(index))
    }

    fn fact(&self, context: &K, anchor: A) -> Option<&V> {
        let index = self.reached(context)?;
        self.env.environment(index).ok()?.get(anchor)
    }

    fn facts(&self, context: &K) -> impl Iterator<Item = (A, &V)> {
        self.reached(context)
            .and_then(|index| self.env.environment(index).ok())
            .into_iter()
            .flat_map(|facts| facts.iter())
    }

    fn response(&self, context: &K) -> Option<Product<V>> {
        self.contexts.get(&self.reached(context)?)?.1.clone()
    }
}

impl<K, V> Analyzed<K, SSAValue, V, StrongDemand>
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
