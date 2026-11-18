//! Reading converged analysis facts: [`AnalysisResults`] is what an abstract
//! engine's `analyze` returns, and every read is a method on it.

use std::collections::HashMap;
use std::hash::Hash;
use std::marker::PhantomData;

use kirin_ir::{CompileStage, Lattice, Product};

use crate::{EnvIndex, EnvStore, FactStore, LatticeAnchor, LinkTarget};

/// The owned result of one converged analysis: facts of context key `K`
/// attached at anchor `A` with lattice value `V`, computed under semantic key
/// `Sem`. It holds no engine and no borrow of the program.
///
/// Only `analyze` builds one, so it always has a root, and it cannot run
/// again: analyzing another root takes another engine. Program edits do not
/// update its facts.
///
/// A *context* is one analyzed copy of a callable under specific boundary
/// facts. Facts are read at three levels: per context, per callable (joined
/// over the callable's contexts), and per stage (joined over the stage's
/// contexts). The *root* is the context `analyze` was asked for; it is read
/// like any other context. Joined answers cover the contexts **reached from
/// the root**. They say nothing about callers the analysis never visited.
pub struct AnalysisResults<K, A: LatticeAnchor, V, Sem> {
    root: K,
    /// The converged fact store, moved out of the engine.
    env: EnvStore<K, A, V>,
    /// Each reached context's callable and response.
    contexts: HashMap<EnvIndex, (LinkTarget, Option<Product<V>>)>,
    /// The environments of each reached callable's contexts.
    by_callable: HashMap<LinkTarget, Vec<EnvIndex>>,
    /// The environments of the reached contexts at each stage.
    by_stage: HashMap<CompileStage, Vec<EnvIndex>>,
    /// Keeps two semantic keys that share an engine distinct types.
    _semantics: PhantomData<fn() -> Sem>,
}

impl<K: Clone + Eq + Hash, A: LatticeAnchor, V, Sem> AnalysisResults<K, A, V, Sem> {
    /// Take ownership of a converged engine's fact store, with each reached
    /// context's environment, callable and response. Every engine builds its
    /// result here, including a downstream engine.
    ///
    /// `root` is the context `analyze` was asked for; `env` holds every
    /// context's facts, each environment registered under its context key;
    /// `contexts` lists each reached context once, as its environment in
    /// `env`, the callable it analyzes, and its response (`None` if it has no
    /// answer). Keyed environments left out of `contexts` are freed here.
    ///
    /// Panics if `root` is not a reached context.
    pub fn new(
        root: K,
        mut env: EnvStore<K, A, V>,
        contexts: impl IntoIterator<Item = (EnvIndex, LinkTarget, Option<Product<V>>)>,
    ) -> Self {
        let mut by_callable: HashMap<LinkTarget, Vec<EnvIndex>> = HashMap::new();
        let mut by_stage: HashMap<CompileStage, Vec<EnvIndex>> = HashMap::new();
        let contexts: HashMap<_, _> = contexts
            .into_iter()
            .map(|(index, callable, response)| {
                by_callable.entry(callable).or_default().push(index);
                by_stage.entry(callable.stage).or_default().push(index);
                (index, (callable, response))
            })
            .collect();
        assert!(
            env.context_env(&root)
                .is_some_and(|index| contexts.contains_key(&index)),
            "the root must be a reached context"
        );
        let unreached: Vec<_> = env
            .contexts()
            .filter_map(|(_, index)| (!contexts.contains_key(&index)).then_some(index))
            .collect();
        for index in unreached {
            env.free(index).expect("context environments are live");
        }
        Self {
            root,
            env,
            contexts,
            by_callable,
            by_stage,
            _semantics: PhantomData,
        }
    }
}

impl<K, A, V, Sem> AnalysisResults<K, A, V, Sem>
where
    K: Clone + Eq + Hash,
    A: LatticeAnchor,
    V: Clone + Lattice,
{
    /// The context `analyze` was asked for.
    pub fn root(&self) -> &K {
        &self.root
    }

    /// Every context reached from the root, the root included, in no
    /// particular order.
    pub fn contexts(&self) -> impl Iterator<Item = &K> {
        self.env.contexts().map(|(key, _)| key)
    }

    /// Which callable `context` is a copy of; `None` if the analysis did not
    /// reach `context`.
    pub fn callable_of(&self, context: &K) -> Option<LinkTarget> {
        Some(self.contexts.get(&self.reached(context)?)?.0)
    }

    /// The fact at `anchor` in `context`. `None` means absent, which is
    /// bottom.
    pub fn fact(&self, context: &K, anchor: A) -> Option<&V> {
        self.env
            .environment(self.reached(context)?)
            .ok()?
            .get(anchor)
    }

    /// Every stored fact of `context`.
    pub fn facts(&self, context: &K) -> impl Iterator<Item = (A, &V)> {
        self.reached(context)
            .and_then(|index| self.env.environment(index).ok())
            .into_iter()
            .flat_map(|facts| facts.iter())
    }

    /// What `context` answers its caller, one fact per slot. `None` means it
    /// has no answer, which differs from an empty one.
    pub fn response(&self, context: &K) -> Option<Product<V>> {
        self.contexts.get(&self.reached(context)?)?.1.clone()
    }

    /// Every callable reached, each once, in no particular order.
    pub fn callables(&self) -> impl Iterator<Item = LinkTarget> {
        self.by_callable.keys().copied()
    }

    /// The contexts that analyze `callable`.
    pub fn contexts_of(&self, callable: LinkTarget) -> impl Iterator<Item = &K> {
        self.by_callable
            .get(&callable)
            .into_iter()
            .flatten()
            .filter_map(|&index| self.env.key(index))
    }

    /// The join of the fact at `anchor` over `callable`'s contexts. `None`
    /// means absent in all of them.
    pub fn joined_fact(&self, callable: LinkTarget, anchor: A) -> Option<V> {
        self.join_at(self.by_callable.get(&callable), anchor)
    }

    /// The join of the fact at `anchor` over every context at `stage`, so a
    /// reader need not know which callable `anchor` belongs to. Anchors are
    /// allocated per stage, so `anchor` names one IR entity only together
    /// with its stage; that entity belongs to one callable, whose contexts
    /// are the only ones with a fact for it. `None` means absent in all of
    /// them.
    pub fn joined_fact_at(&self, stage: CompileStage, anchor: A) -> Option<V> {
        self.join_at(self.by_stage.get(&stage), anchor)
    }

    /// The join of every stored fact over `callable`'s contexts.
    pub fn joined_facts(&self, callable: LinkTarget) -> FactStore<A, V> {
        let mut joined: FactStore<A, V> = FactStore::new();
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
    pub fn joined_response(&self, callable: LinkTarget) -> Option<Product<V>> {
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

    /// The environment of `key`, if the analysis reached it.
    fn reached(&self, key: &K) -> Option<EnvIndex> {
        self.env.context_env(key)
    }

    /// The join of the fact at `anchor` over the environments of one index
    /// entry; `None` for a missing entry.
    fn join_at(&self, entry: Option<&Vec<EnvIndex>>, anchor: A) -> Option<V> {
        entry
            .into_iter()
            .flatten()
            .filter_map(|&index| self.env.environment(index).ok()?.get(anchor.clone()))
            .cloned()
            .reduce(|joined, fact| joined.join(&fact))
    }
}
