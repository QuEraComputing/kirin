//! Liveness results: the sparse demand set (strong liveness) and the dense
//! per-point sets (classic liveness), plus their composition.

use kirin_interpreter::{
    Body, BodyScope, DenseBackwardInterpreter, FactStore, InterpreterError, ProgramPoint, Scoped,
    SparseBackwardInterpreter,
};
use kirin_ir::{CompileStage, Lattice, SSAValue, StageMeta};

use crate::live::{Live, LiveSet};

/// The result of [`analyze_demand`](crate::analyze_demand): the demanded SSA
/// values.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DemandResult {
    demanded: LiveSet,
}

impl DemandResult {
    pub(crate) fn from_engine<S: StageMeta>(
        engine: &SparseBackwardInterpreter<'_, S, Live, InterpreterError>,
        stage: CompileStage,
        body: impl Into<Body>,
    ) -> Self {
        // The engine's sparse fact view; the demand set is its live support.
        let facts = engine.fact_store(stage, body);
        let demanded = facts
            .iter()
            .filter(|(_, fact)| fact.is_live())
            .map(|(value, _)| value)
            .collect();
        Self { demanded }
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
    facts: FactStore<Scoped<BodyScope, ProgramPoint>, LiveSet>,
}

impl DenseLivenessResult {
    /// Copy the facts recorded by a converged dense engine.
    pub fn from_engine<'ir, S, F>(
        engine: &DenseBackwardInterpreter<'ir, S, LiveSet, InterpreterError, F>,
    ) -> Self
    where
        S: StageMeta,
    {
        Self {
            facts: engine.facts(),
        }
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
