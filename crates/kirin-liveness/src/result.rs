//! Liveness results: the sparse demand set (strong liveness) and the dense
//! per-point sets (classic liveness), plus their composition.

use kirin_interpreter::{
    Body, DenseBackwardCompletion, DenseBackwardDriver, DenseBackwardInterpreter,
    DenseBackwardTransfer, DenseBlockStore, DenseFrameBuild, DensePointStore, Frame,
    InterpDispatch, InterpreterError, ProgramPoint, SparseBackwardInterpreter, StageQuery,
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

/// The result of [`analyze_dense`](crate::analyze_dense): classic per-point
/// liveness — block-boundary sets plus reconstructed per-statement sets.
///
/// These sets carry the conventional (regalloc-grade) meaning: every use gens,
/// purity-irrelevant. Strong per-point sets are the composition
/// [`strong_live_before`](Self::strong_live_before) — the classic set
/// intersected with the demand set.
#[derive(Clone, Debug)]
pub struct DenseLivenessResult {
    blocks: DenseBlockStore<LiveSet>,
    points: DensePointStore<LiveSet>,
}

impl DenseLivenessResult {
    /// Build the result from a converged dense engine: copy the boundary
    /// summaries and reconstruct every per-statement state by replaying each
    /// block through the dialect rules.
    pub fn from_engine<'ir, S, F>(
        engine: &mut DenseBackwardInterpreter<'ir, S, LiveSet, InterpreterError, F>,
        stage: CompileStage,
        body: impl Into<Body>,
    ) -> Result<Self, InterpreterError>
    where
        S: StageMeta
            + StageQuery
            + InterpDispatch<DenseBackwardTransfer<'ir, S, LiveSet, InterpreterError, F>>,
        F: Frame<
                DenseBackwardDriver<'ir, S, LiveSet, InterpreterError, F>,
                Completion = DenseBackwardCompletion<LiveSet>,
            > + DenseFrameBuild<LiveSet, InterpreterError>,
    {
        let body = body.into();
        let mut blocks = DenseBlockStore::new();
        for block in engine.cfg_blocks() {
            if let Some(summary) = engine.block_summary(stage, body, block) {
                blocks.set_entry(block, summary.live_in.clone());
                blocks.set_exit(block, summary.live_out.clone());
            }
        }
        let points = engine.reconstruct_points(stage, body)?;
        Ok(Self { blocks, points })
    }

    /// Iterate `(block, live_in, live_out)` triples (order unspecified).
    pub fn blocks(&self) -> impl Iterator<Item = (Block, &LiveSet, &LiveSet)> {
        self.blocks.entries().filter_map(|(block, live_in)| {
            self.blocks
                .exit(block)
                .map(|live_out| (block, live_in, live_out))
        })
    }

    /// The set of values live on entry to `block`.
    pub fn live_in(&self, block: Block) -> Option<&LiveSet> {
        self.blocks.entry(block)
    }

    /// The set of values live on exit from `block` (excludes the terminator's
    /// own uses, e.g. the branch condition).
    pub fn live_out(&self, block: Block) -> Option<&LiveSet> {
        self.blocks.exit(block)
    }

    /// The set of values live immediately before `statement`.
    pub fn live_before(&self, statement: Statement) -> Option<&LiveSet> {
        self.points.get(ProgramPoint::Before(statement))
    }

    /// The set of values live immediately after `statement`.
    pub fn live_after(&self, statement: Statement) -> Option<&LiveSet> {
        self.points.get(ProgramPoint::After(statement))
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
