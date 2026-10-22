//! Liveness results: the sparse demand set (strong liveness) and the dense
//! per-point sets (classic liveness), plus their composition.

use std::collections::HashMap;
use std::hash::Hash;

use kirin_interpreter::{
    BodyScope, DemandContext, DenseBackwardInterpreter, FactStore, InterpreterError, LinkTarget,
    ProgramPoint, Scoped, SparseBackwardInterpreter,
};
use kirin_ir::{Lattice, SSAValue, StageMeta};

use crate::DemandCallCtx;
use crate::live::{Demand, LiveSet};

/// The result of [`analyze_demand`](crate::analyze_demand): the demanded SSA
/// values of every callable context the analysis reached, keyed by the
/// engine's context key `K` (default: [`LinkTarget`] plus [`DemandCallCtx`]).
///
/// The root-scoped accessors ([`demanded`](Self::demanded),
/// [`is_demanded`](Self::is_demanded)) read the analyzed root's context;
/// [`contexts`](Self::contexts) lists every reached context.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DemandResult<K = (LinkTarget, DemandCallCtx)>
where
    K: Eq + Hash,
{
    root: K,
    contexts: HashMap<K, (BodyScope, LiveSet)>,
}

impl<K: Clone + Eq + Hash> DemandResult<K> {
    /// Copy the demanded sets of every context a converged engine discovered.
    pub fn from_engine<S, E, Lk, P>(
        engine: &SparseBackwardInterpreter<'_, S, Demand, E, Lk, P>,
    ) -> Result<Self, InterpreterError>
    where
        S: StageMeta,
        E: From<InterpreterError>,
        P: DemandContext<Demand, Key = K>,
    {
        let root = engine.root().cloned().ok_or(InterpreterError::Custom(
            "demand analysis has no successful root",
        ))?;
        let mut contexts = HashMap::new();
        for context in engine.contexts() {
            let scope = engine.scope(context).ok_or(InterpreterError::Custom(
                "discovered demand context has no scope",
            ))?;
            let demanded = engine
                .facts(context)
                .filter(|(_, fact)| fact.is_live())
                .map(|(value, _)| value)
                .collect();
            contexts.insert(context.clone(), (scope, demanded));
        }
        if !contexts.contains_key(&root) {
            return Err(InterpreterError::Custom(
                "demand root context was never discovered",
            ));
        }
        Ok(Self { root, contexts })
    }

    /// The analyzed root's context key.
    pub fn root(&self) -> &K {
        &self.root
    }

    /// The resolved `(target stage, callable body)` analyzed by the root.
    pub fn root_scope(&self) -> BodyScope {
        self.contexts[&self.root].0
    }

    /// The root's demanded SSA values (the strong-liveness fact).
    pub fn demanded(&self) -> &LiveSet {
        &self.contexts[&self.root].1
    }

    /// `true` iff `value` is transitively needed by a root in the root's
    /// context.
    pub fn is_demanded(&self, value: impl Into<SSAValue>) -> bool {
        self.demanded().contains(value.into())
    }

    /// Every context the analysis reached, in no particular order.
    pub fn contexts(&self) -> impl Iterator<Item = &K> {
        self.contexts.keys()
    }
}

/// The result of [`analyze_dense`](crate::analyze_dense): classic liveness at
/// every block and statement program point.
///
/// These sets carry the conventional (regalloc-grade) meaning: every use gens,
/// purity-irrelevant. Strong per-point sets are
/// [`strong_point_facts`](Self::strong_point_facts): the classic set
/// intersected with the demand set.
#[derive(Clone, Debug)]
pub struct DenseLivenessResult {
    root_scope: BodyScope,
    facts: FactStore<Scoped<BodyScope, ProgramPoint>, LiveSet>,
}

impl DenseLivenessResult {
    /// Copy the facts recorded by a converged dense engine.
    pub fn from_engine<'ir, S, F, Lk>(
        engine: &DenseBackwardInterpreter<'ir, S, LiveSet, InterpreterError, F, Lk>,
        root_scope: BodyScope,
    ) -> Self
    where
        S: StageMeta,
    {
        Self {
            root_scope,
            facts: engine.facts(),
        }
    }

    /// The resolved `(target stage, callable body)` analyzed by this result.
    pub fn root_scope(&self) -> BodyScope {
        self.root_scope
    }

    /// The liveness fact recorded at `point`.
    pub fn point_facts(&self, point: Scoped<BodyScope, ProgramPoint>) -> Option<&LiveSet> {
        self.facts.get(point)
    }

    /// Strong fact at `point`: the classic set intersected with the demand set
    /// (values live there *and* transitively needed by a root).
    pub fn strong_point_facts<K: Clone + Eq + Hash>(
        &self,
        point: Scoped<BodyScope, ProgramPoint>,
        demand: &DemandResult<K>,
    ) -> Option<LiveSet> {
        self.point_facts(point)
            .map(|set| set.meet(demand.demanded()))
    }
}
