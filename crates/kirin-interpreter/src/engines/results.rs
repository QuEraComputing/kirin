//! Reading converged analysis facts: [`Analyzed`] is what an abstract
//! engine's `analyze` returns, and [`AnalysisResults`] is the one read-only
//! view all three engines implement on it.

use std::collections::{HashMap, HashSet};

use kirin_ir::{Lattice, Product};

use crate::{EnvIndex, FactStore, LatticeAnchor, LinkTarget};

/// A converged analysis: the state of engine `I` after one `analyze` call.
///
/// Only `analyze` builds one, so it always has a root, and it cannot run
/// again: analyzing another root takes another engine. Read it through
/// [`AnalysisResults`].
pub struct Analyzed<I> {
    pub(crate) engine: I,
    /// The environments of each reached callable's contexts. Built once:
    /// the analysis cannot run again, so it never goes stale.
    pub(crate) by_callable: HashMap<LinkTarget, Vec<EnvIndex>>,
}

impl<I> Analyzed<I> {
    /// Wrap a converged `engine`, indexing its `contexts` (each reached
    /// context's callable and environment) by callable.
    pub(crate) fn new(
        engine: I,
        contexts: impl IntoIterator<Item = (LinkTarget, EnvIndex)>,
    ) -> Self {
        let mut by_callable: HashMap<LinkTarget, Vec<EnvIndex>> = HashMap::new();
        for (callable, env) in contexts {
            by_callable.entry(callable).or_default().push(env);
        }
        Self {
            engine,
            by_callable,
        }
    }

    /// The environments of `callable`'s contexts; empty if it was not reached.
    pub(crate) fn envs_of(&self, callable: LinkTarget) -> impl Iterator<Item = EnvIndex> {
        self.by_callable
            .get(&callable)
            .into_iter()
            .flatten()
            .copied()
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
