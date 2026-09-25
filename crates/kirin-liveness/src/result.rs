//! Liveness results: the sparse demand set (strong liveness) and the dense
//! per-point sets (classic liveness), plus their composition.

use kirin_interpreter::{
    BodyScope, DenseBackwardInterpreter, FactStore, InterpreterError, ProgramPoint, Scoped,
    SparseBackwardInterpreter,
};
use kirin_ir::{Lattice, SSAValue, StageMeta};

use crate::live::{Live, LiveSet};

/// The result of [`analyze_demand`](crate::analyze_demand): the demanded SSA
/// values.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DemandResult {
    root_scope: BodyScope,
    demanded: LiveSet,
}

impl DemandResult {
    pub(crate) fn from_engine<S: StageMeta, Lk>(
        engine: &SparseBackwardInterpreter<'_, S, Live, InterpreterError, Lk>,
        root_scope: BodyScope,
    ) -> Self {
        // The engine's sparse fact view; the demand set is its live support.
        let facts = engine.fact_store(root_scope.0, root_scope.1);
        let demanded = facts
            .iter()
            .filter(|(_, fact)| fact.is_live())
            .map(|(value, _)| value)
            .collect();
        Self {
            root_scope,
            demanded,
        }
    }

    /// The resolved `(target stage, callable body)` analyzed by this result.
    pub fn root_scope(&self) -> BodyScope {
        self.root_scope
    }

    /// The demanded SSA values (the strong-liveness fact).
    pub fn demanded(&self) -> &LiveSet {
        &self.demanded
    }

    /// `true` iff `value` is transitively needed by a root.
    pub fn is_demanded(&self, value: impl Into<SSAValue>) -> bool {
        self.demanded.contains(value.into())
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
    pub fn strong_point_facts(
        &self,
        point: Scoped<BodyScope, ProgramPoint>,
        demand: &DemandResult,
    ) -> Option<LiveSet> {
        self.point_facts(point)
            .map(|set| set.meet(demand.demanded()))
    }
}
