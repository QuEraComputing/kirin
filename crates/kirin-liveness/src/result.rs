//! Liveness results: the sparse demand set (strong liveness) and the dense
//! per-point sets (classic liveness), plus their composition.

use kirin_interpreter::{
    Body, DenseBackwardInterpreter, DenseFactStore, InterpreterError, ProgramPoint,
    SparseBackwardInterpreter,
};
use kirin_ir::{Block, CompileStage, Lattice, SSAValue, StageMeta, Statement};

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
/// purity-irrelevant. Strong per-point sets are the composition
/// [`strong_live_before`](Self::strong_live_before) — the classic set
/// intersected with the demand set.
#[derive(Clone, Debug)]
pub struct DenseLivenessResult {
    facts: DenseFactStore<LiveSet>,
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
            facts: engine.fact_store().clone(),
        }
    }

    /// The liveness fact recorded at `point`.
    pub fn point_facts(&self, point: ProgramPoint) -> Option<&LiveSet> {
        self.facts.get(point)
    }

    /// Iterate `(block, live_in, live_out)` triples (order unspecified).
    pub fn blocks(&self) -> impl Iterator<Item = (Block, &LiveSet, &LiveSet)> {
        self.facts.iter().filter_map(|(point, live_in)| {
            let ProgramPoint::BlockEntry(block) = point else {
                return None;
            };
            self.facts
                .get(ProgramPoint::BlockExit(block))
                .map(|live_out| (block, live_in, live_out))
        })
    }

    /// The set of values live on entry to `block`.
    pub fn live_in(&self, block: Block) -> Option<&LiveSet> {
        self.point_facts(ProgramPoint::BlockEntry(block))
    }

    /// The set of values live on exit from `block` (excludes the terminator's
    /// own uses, e.g. the branch condition).
    pub fn live_out(&self, block: Block) -> Option<&LiveSet> {
        self.point_facts(ProgramPoint::BlockExit(block))
    }

    /// The set of values live immediately before `statement`.
    pub fn live_before(&self, statement: Statement) -> Option<&LiveSet> {
        self.point_facts(ProgramPoint::Before(statement))
    }

    /// The set of values live immediately after `statement`.
    pub fn live_after(&self, statement: Statement) -> Option<&LiveSet> {
        self.point_facts(ProgramPoint::After(statement))
    }

    /// Strong per-point set: the classic set intersected with the demand set
    /// (values live here *and* transitively needed by a root).
    pub fn strong_live_before(
        &self,
        statement: Statement,
        demand: &DemandResult,
    ) -> Option<LiveSet> {
        self.live_before(statement)
            .map(|set| set.meet(demand.demanded()))
    }

    /// See [`strong_live_before`](Self::strong_live_before).
    pub fn strong_live_after(
        &self,
        statement: Statement,
        demand: &DemandResult,
    ) -> Option<LiveSet> {
        self.live_after(statement)
            .map(|set| set.meet(demand.demanded()))
    }
}
