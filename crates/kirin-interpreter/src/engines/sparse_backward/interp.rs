//! The sparse backward demand engine (strong liveness / neededness).
//!
//! # The analysis
//!
//! A **sparse backward** fact is one lattice element per SSA value: *how
//! strongly is this value needed?* For strong (true) liveness the lattice is
//! two-point (demanded or not): a value is demanded iff it transitively feeds
//! a *root* — a return/terminator operand or an impure statement. Dead code
//! demands nothing. The fact does not vary by program point, so it anchors to
//! [`SSAValue`]s; per-point live sets are the dense analysis's product
//! (`DenseBackward`), not this one's.
//!
//! # Layering (owner-summary fixpoint)
//!
//! [`SparseBackwardInterpreter`] is the public engine. Internally it is a
//! wrapper over a [`StandardFixpointInterpreter`] driving a summary-free
//! [`SparseBackwardTransfer`]:
//!
//! - **[`SparseBackwardTransfer`]** is the [`Interp`] delegate: pipeline access,
//!   the real dispatch location, and the per-rule demand buffer.
//! - the **[`StandardFixpointInterpreter`]** driver owns the demand facts
//!   (summaries keyed by [`Scoped`] SSA values — never bare values), the value
//!   worklist, and the analysis state (scope + region topology).
//!
//! # Owners are values; scheduling is demand propagation
//!
//! `SummaryKey = Scoped<(CompileStage, Region), SSAValue>`: the fact anchor is
//! the owner. The driver's *default* self-dependent index
//! ([`OwnerSummaryDeps`]) is exactly demand propagation — a value whose fact
//! rises is rescheduled, and analyzing a value means dispatching the rules
//! that can translate its demand:
//!
//! - a statement **result** → the defining statement's backward rule;
//! - a **block argument** → each of the block's *feeders* (terminators
//!   targeting the block, statements owning it as a structured body) from the
//!   [`RegionTopology`];
//! - a graph **port** → unsupported (loud error).
//!
//! Rules read converged facts ([`SparseBackwardInterp::is_demanded`]) and
//! raise new demands ([`SparseBackwardInterp::demand`]); each rule returns the
//! demands it raised as its [`SparseBackwardEffect`]. All fact mutation flows
//! through the driver's single merge path. Facts only rise in a finite-height
//! lattice, so the fixpoint terminates with O(feeders) rule runs per rise —
//! no block re-walks, no widening, and no frames for structured control:
//! loop-carried demand (e.g. `scf.for`) converges through the value worklist.

use std::marker::PhantomData;
use std::mem;

use kirin_ir::{
    Block, CompileStage, HasArguments, HasBottom, HasResults, HasTop, IsPure, Lattice, Pipeline,
    Region, SSAKind, SSAValue, StageMeta, Statement,
};

use crate::{
    AbstractInterpreter, EnvIndex, FixpointProfile, Frame, FrameEffect, Interp, InterpDispatch,
    InterpLocation, InterpreterError, OwnerSemantics, OwnerSummaryDeps, RegionTopology, Scoped,
    SparseBackward, SparseStore, StageQuery, StandardFixpointInterpreter, Summary, SummaryEffect,
    query,
};

/// The scope a region-level backward analysis qualifies its facts with.
///
/// Arena ids are per-stage, so the stage is part of the scope; analyzing two
/// regions in one engine keeps their facts distinct.
pub type RegionScope = (CompileStage, Region);

// ===========================================================================
// Effect + dialect-facing trait
// ===========================================================================

/// What a backward rule produced: the demand facts it raised.
///
/// Rules raise demands imperatively through
/// [`demand`](SparseBackwardInterp::demand) and finish with
/// [`effect()`](SparseBackwardInterp::effect), which drains the buffer into
/// this value; the driver applies it through its single merge path.
pub enum SparseBackwardEffect<V> {
    /// Demand facts to merge (possibly empty).
    Demands(Vec<(SSAValue, V)>),
}

/// Sparse-backward engine flavor: demand-fact access plus
/// [`SparseBackwardEffect`].
///
/// Backward demand rules (`impl Interpretable<I, SparseBackward>`) bound on
/// this trait. A rule reads converged facts with
/// [`fact`](Self::fact)/[`is_demanded`](Self::is_demanded), raises demands
/// with [`demand`](Self::demand), and returns [`effect()`](Self::effect).
/// Rules are scope-blind: they name bare SSA values, and the engine qualifies
/// them with the current analysis scope.
pub trait SparseBackwardInterp:
    Interp<Kind = SparseBackward, Effect = SparseBackwardEffect<<Self as Interp>::Value>>
{
    /// The converged demand fact for `value` (bottom if absent).
    fn fact(&self, value: impl Into<SSAValue>) -> Result<Self::Value, Self::Error>;

    /// Raise `value`'s demand to ⊤ (buffered; returned by [`effect`](Self::effect)).
    fn demand(&mut self, value: impl Into<SSAValue>) -> Result<(), Self::Error>;

    /// Drain the demand buffer into this rule's effect.
    fn effect(&mut self) -> Self::Effect;

    /// The parameters of `block` (for mapping parameter demand onto edge or
    /// carried arguments in terminator/structured rules).
    fn block_params(&self, block: Block) -> Result<Vec<SSAValue>, Self::Error>;

    /// The operands of `block`'s terminator — a structured body's yield slots.
    fn terminator_args(&self, block: Block) -> Result<Vec<SSAValue>, Self::Error>;

    /// `true` iff `value` carries a non-bottom demand fact.
    fn is_demanded(&self, value: impl Into<SSAValue>) -> Result<bool, Self::Error>
    where
        Self::Value: HasBottom + PartialEq,
    {
        Ok(self.fact(value)? != Self::Value::bottom())
    }

    /// Purity-aware neededness transfer for an ordinary (frame-free)
    /// statement: demand the operands iff the statement is impure or any of
    /// its results is demanded. There is no kill — demand only rises, and SSA
    /// needs no scoping.
    fn transfer_ordinary<T>(&mut self, stmt: &T) -> Result<Self::Effect, Self::Error>
    where
        T: for<'a> HasArguments<'a> + for<'a> HasResults<'a> + IsPure,
        Self::Value: HasBottom + PartialEq,
    {
        let mut live = false;
        for result in stmt.results() {
            if self.is_demanded(*result)? {
                live = true;
                break;
            }
        }
        if !stmt.is_pure() || live {
            for argument in stmt.arguments() {
                self.demand(*argument)?;
            }
        }
        Ok(self.effect())
    }
}

// ===========================================================================
// SparseBackwardTransfer — the summary-free Interp delegate
// ===========================================================================

/// The summary-free transfer of the sparse backward engine: pipeline access,
/// the real dispatch location, and the per-rule demand buffer.
pub struct SparseBackwardTransfer<'ir, S: StageMeta, V, E> {
    pipeline: &'ir Pipeline<S>,
    location: Option<InterpLocation>,
    /// Demands the currently dispatched rule has raised.
    demands: Vec<(SSAValue, V)>,
    /// The analysis's single activation: sparse backward facts live in one
    /// scope-qualified store, not per-call activation records.
    activation: EnvIndex,
    _marker: PhantomData<fn() -> E>,
}

impl<'ir, S: StageMeta, V, E> SparseBackwardTransfer<'ir, S, V, E> {
    fn new(pipeline: &'ir Pipeline<S>) -> Self {
        Self {
            pipeline,
            location: None,
            demands: Vec::new(),
            activation: EnvIndex::new(0),
            _marker: PhantomData,
        }
    }

    pub fn pipeline(&self) -> &'ir Pipeline<S> {
        self.pipeline
    }
}

impl<'ir, S, V, E> Interp for SparseBackwardTransfer<'ir, S, V, E>
where
    S: StageMeta,
    V: Clone,
    E: From<InterpreterError>,
{
    type Value = V;
    type Error = E;
    type Effect = SparseBackwardEffect<V>;
    type Kind = SparseBackward;

    fn stage(&self) -> CompileStage {
        self.location.expect("interp location not set").stage
    }

    fn statement(&self) -> Statement {
        self.location.expect("interp location not set").statement
    }

    fn index(&self) -> EnvIndex {
        self.activation
    }
}

impl<'ir, S, V, E> AbstractInterpreter for SparseBackwardTransfer<'ir, S, V, E>
where
    S: StageMeta,
    V: Clone,
    E: From<InterpreterError>,
{
}

// ===========================================================================
// Summary + profile + analysis state
// ===========================================================================

/// The per-value summary: one demand fact. Its merge is a real lattice join —
/// `Some(())` iff the fact rose — which is what drives rescheduling.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DemandSummary<V>(pub V);

impl<V> Summary for DemandSummary<V>
where
    V: Clone + PartialEq + Lattice,
{
    type Strategy = ();
    type Change = ();

    fn merge(
        &mut self,
        _phase: crate::FixpointPhase,
        candidate: Self,
        _strategy: &mut Self::Strategy,
    ) -> Option<Self::Change> {
        let joined = self.0.join(&candidate.0);
        if joined == self.0 {
            None
        } else {
            self.0 = joined;
            Some(())
        }
    }
}

/// The owner-summary type family for the sparse backward engine.
pub struct SparseBackwardProfile<V, E> {
    _marker: PhantomData<fn() -> (V, E)>,
}

impl<'ir, S, V, E> FixpointProfile<SparseBackwardTransfer<'ir, S, V, E>>
    for SparseBackwardProfile<V, E>
where
    S: StageMeta,
    V: Clone + PartialEq + Lattice,
    E: From<InterpreterError>,
{
    type SummaryKey = Scoped<RegionScope, SSAValue>;
    type Summary = DemandSummary<V>;
    type Frame = DemandFrame<V>;
    type Completion = Vec<(SSAValue, V)>;
}

/// Analysis-local state carried in the driver's `store` slot: the scope facts
/// are qualified with, and the region topology (feeders for block arguments).
#[derive(Default)]
pub struct BackwardAnalysisState {
    scope: Option<RegionScope>,
    topology: RegionTopology,
}

/// The sparse backward driver: a [`StandardFixpointInterpreter`] over
/// [`SparseBackwardTransfer`] with scope-qualified value owners and the
/// default self-dependent index (demand propagation).
pub type SparseBackwardDriver<'ir, S, V, E> = StandardFixpointInterpreter<
    SparseBackwardTransfer<'ir, S, V, E>,
    SparseBackwardProfile<V, E>,
    BackwardAnalysisState,
    OwnerSummaryDeps<Scoped<RegionScope, SSAValue>>,
>;

// ===========================================================================
// The demand frame: one owner analysis = dispatch the translating rules
// ===========================================================================

/// One value-owner analysis: dispatch each statement whose rule can translate
/// the owner's demand, accumulating the demands the rules raise.
pub struct DemandFrame<V> {
    stage: CompileStage,
    work: Vec<Statement>,
    collected: Vec<(SSAValue, V)>,
}

impl<V> DemandFrame<V> {
    fn new(stage: CompileStage, work: Vec<Statement>) -> Self {
        Self {
            stage,
            work,
            collected: Vec::new(),
        }
    }
}

impl<'ir, S, V, E> Frame<SparseBackwardDriver<'ir, S, V, E>> for DemandFrame<V>
where
    S: StageMeta + StageQuery + InterpDispatch<SparseBackwardDriver<'ir, S, V, E>, SparseBackward>,
    V: Clone + PartialEq + Lattice + HasBottom + HasTop,
    E: From<InterpreterError>,
{
    type Completion = Vec<(SSAValue, V)>;

    fn step(
        mut self,
        interp: &mut SparseBackwardDriver<'ir, S, V, E>,
    ) -> Result<FrameEffect<Self, Self::Completion>, E> {
        match self.work.pop() {
            Some(statement) => {
                let SparseBackwardEffect::Demands(demands) =
                    interp.run_statement(self.stage, statement)?;
                self.collected.extend(demands);
                Ok(FrameEffect::Continue(self))
            }
            None => Ok(FrameEffect::Complete(self.collected)),
        }
    }

    fn resume_done(
        self,
        _interp: &mut SparseBackwardDriver<'ir, S, V, E>,
    ) -> Result<FrameEffect<Self, Self::Completion>, E> {
        Err(E::from(InterpreterError::Custom(
            "demand frames never push children",
        )))
    }

    fn resume(
        self,
        _completion: Self::Completion,
        _interp: &mut SparseBackwardDriver<'ir, S, V, E>,
    ) -> Result<FrameEffect<Self, Self::Completion>, E> {
        Err(E::from(InterpreterError::Custom(
            "demand frames never push children",
        )))
    }
}

// ===========================================================================
// Driver capabilities: dispatch + the dialect-facing trait
// ===========================================================================

impl<'ir, S, V, E> SparseBackwardDriver<'ir, S, V, E>
where
    S: StageMeta + StageQuery + InterpDispatch<Self, SparseBackward>,
    V: Clone + PartialEq + Lattice,
    E: From<InterpreterError>,
{
    /// Dispatch one statement's backward rule with the location set.
    fn run_statement(
        &mut self,
        stage: CompileStage,
        statement: Statement,
    ) -> Result<SparseBackwardEffect<V>, E> {
        let pipeline = self.inner().pipeline();
        let info = pipeline
            .stage(stage)
            .ok_or_else(|| E::from(InterpreterError::MissingStage(stage)))?;
        let activation = self.inner().activation;
        let previous = self.inner_mut().location.replace(InterpLocation {
            stage,
            statement,
            index: activation,
        });
        let result = info.dispatch_statement(statement, self);
        self.inner_mut().location = previous;
        result
    }
}

impl<'ir, S, V, E> SparseBackwardInterp for SparseBackwardDriver<'ir, S, V, E>
where
    S: StageMeta + StageQuery,
    V: Clone + PartialEq + Lattice + HasBottom + HasTop,
    E: From<InterpreterError>,
{
    fn fact(&self, value: impl Into<SSAValue>) -> Result<V, E> {
        let scope = self
            .store()
            .scope
            .ok_or_else(|| E::from(InterpreterError::Custom("no active backward analysis")))?;
        Ok(self
            .summary(&Scoped::new(scope, value.into()))
            .map(|summary| summary.0.clone())
            .unwrap_or_else(V::bottom))
    }

    fn demand(&mut self, value: impl Into<SSAValue>) -> Result<(), E> {
        let value = value.into();
        self.inner_mut().demands.push((value, V::top()));
        Ok(())
    }

    fn effect(&mut self) -> SparseBackwardEffect<V> {
        SparseBackwardEffect::Demands(mem::take(&mut self.inner_mut().demands))
    }

    fn block_params(&self, block: Block) -> Result<Vec<SSAValue>, E> {
        query::block_params(self.inner().pipeline(), self.stage(), block).map_err(E::from)
    }

    fn terminator_args(&self, block: Block) -> Result<Vec<SSAValue>, E> {
        query::terminator_arguments(self.inner().pipeline(), self.stage(), block).map_err(E::from)
    }
}

// ===========================================================================
// Owner semantics: analyzing one value = running its translating rules
// ===========================================================================

struct SparseBackwardSemantics;

impl<'ir, S, V, E>
    OwnerSemantics<
        SparseBackwardDriver<'ir, S, V, E>,
        Scoped<RegionScope, SSAValue>,
        DemandSummary<V>,
        DemandFrame<V>,
        Vec<(SSAValue, V)>,
        E,
    > for SparseBackwardSemantics
where
    S: StageMeta + StageQuery + InterpDispatch<SparseBackwardDriver<'ir, S, V, E>, SparseBackward>,
    V: Clone + PartialEq + Lattice + HasBottom + HasTop,
    E: From<InterpreterError>,
{
    fn bottom_summary(
        &mut self,
        _interp: &mut SparseBackwardDriver<'ir, S, V, E>,
        _owner: &Scoped<RegionScope, SSAValue>,
    ) -> Result<DemandSummary<V>, E> {
        Ok(DemandSummary(V::bottom()))
    }

    fn entry_frame(
        &mut self,
        interp: &mut SparseBackwardDriver<'ir, S, V, E>,
        owner: &Scoped<RegionScope, SSAValue>,
        _summary: &DemandSummary<V>,
    ) -> Result<DemandFrame<V>, E> {
        let (stage, _region) = owner.scope;
        let kind = query::value_kind(interp.inner().pipeline(), stage, owner.item)?;
        let work = match kind {
            SSAKind::Result(statement, _) => vec![statement],
            SSAKind::BlockArgument(block, _) => interp.store().topology.feeders(block).to_vec(),
            SSAKind::Port(..) => {
                return Err(E::from(InterpreterError::Custom(
                    "graph ports are not supported by sparse backward demand",
                )));
            }
        };
        Ok(DemandFrame::new(stage, work))
    }

    fn complete_owner(
        &mut self,
        _interp: &mut SparseBackwardDriver<'ir, S, V, E>,
        owner: Scoped<RegionScope, SSAValue>,
        completion: Vec<(SSAValue, V)>,
    ) -> Result<SummaryEffect<Scoped<RegionScope, SSAValue>, DemandSummary<V>>, E> {
        // Scope-qualify the bare values the rules demanded.
        Ok(SummaryEffect::Many(
            completion
                .into_iter()
                .map(|(value, fact)| (Scoped::new(owner.scope, value), DemandSummary(fact)))
                .collect(),
        ))
    }
}

// ===========================================================================
// The public engine
// ===========================================================================

/// Sparse backward demand engine (strong liveness / neededness).
///
/// ```ignore
/// let mut analysis = SparseBackwardInterpreter::<Stage, Live>::new(&pipeline);
/// analysis.analyze(stage, region)?;
/// let demanded = analysis.is_demanded(stage, region, value);
/// ```
pub struct SparseBackwardInterpreter<'ir, S: StageMeta, V, E = InterpreterError>
where
    V: Clone + PartialEq + Lattice,
    E: From<InterpreterError>,
{
    driver: SparseBackwardDriver<'ir, S, V, E>,
}

impl<'ir, S, V, E> SparseBackwardInterpreter<'ir, S, V, E>
where
    S: StageMeta,
    V: Clone + PartialEq + Lattice,
    E: From<InterpreterError>,
{
    pub fn new(pipeline: &'ir Pipeline<S>) -> Self {
        Self {
            driver: StandardFixpointInterpreter::with_dependency_index(
                SparseBackwardTransfer::new(pipeline),
                BackwardAnalysisState::default(),
                (),
                OwnerSummaryDeps::new(),
            ),
        }
    }

    pub fn pipeline(&self) -> &'ir Pipeline<S> {
        self.driver.inner().pipeline()
    }

    /// The converged demand fact for `value` under the `(stage, region)` scope.
    pub fn fact(
        &self,
        stage: CompileStage,
        region: Region,
        value: impl Into<SSAValue>,
    ) -> Option<&V> {
        self.driver
            .summary(&Scoped::new((stage, region), value.into()))
            .map(|summary| &summary.0)
    }

    /// All converged `(value, fact)` pairs under the `(stage, region)` scope.
    pub fn facts(
        &self,
        stage: CompileStage,
        region: Region,
    ) -> impl Iterator<Item = (SSAValue, &V)> {
        let scope = (stage, region);
        self.driver
            .summaries()
            .iter()
            .filter(move |(owner, _)| owner.scope == scope)
            .map(|(owner, summary)| (owner.item, &summary.0))
    }

    /// The converged facts under the `(stage, region)` scope as a
    /// [`SparseStore`] (the sparse per-SSA-value fact view; absent = bottom).
    pub fn fact_store(&self, stage: CompileStage, region: Region) -> SparseStore<V> {
        let mut store = SparseStore::new();
        for (value, fact) in self.facts(stage, region) {
            store.set(value, fact.clone());
        }
        store
    }
}

impl<'ir, S, V, E> SparseBackwardInterpreter<'ir, S, V, E>
where
    S: StageMeta + StageQuery + InterpDispatch<SparseBackwardDriver<'ir, S, V, E>, SparseBackward>,
    V: Clone + PartialEq + Lattice + HasBottom + HasTop,
    E: From<InterpreterError>,
{
    /// Run the demand fixpoint over `region` in `stage`.
    ///
    /// **Prepass**: enumerate the region topology (blocks including structured
    /// bodies, statements, feeders), then run every statement's rule once with
    /// nothing demanded — impure statements and terminators contribute the
    /// demand roots. **Propagation**: drain the value worklist; each risen
    /// value dispatches the rules that translate its demand.
    pub fn analyze(&mut self, stage: CompileStage, region: Region) -> Result<(), E> {
        let scope = (stage, region);
        let topology = query::region_topology(self.driver.inner().pipeline(), stage, region)?;
        let statements: Vec<Statement> = topology
            .blocks
            .iter()
            .flat_map(|block| block.stmts.iter().copied())
            .collect();
        *self.driver.store_mut() = BackwardAnalysisState {
            scope: Some(scope),
            topology,
        };

        let mut semantics = SparseBackwardSemantics;

        // Prepass: collect the demand roots.
        let mut seeds: Vec<(SSAValue, V)> = Vec::new();
        for statement in statements {
            let SparseBackwardEffect::Demands(demands) =
                self.driver.run_statement(stage, statement)?;
            seeds.extend(demands);
        }

        // Propagate to the fixpoint.
        for (value, fact) in seeds {
            self.driver.merge_summary(
                &mut semantics,
                Scoped::new(scope, value),
                DemandSummary(fact),
            )?;
        }
        self.driver.drain_worklist(&mut semantics)
    }

    /// `true` iff `value` carries a non-bottom demand fact under the scope.
    pub fn is_demanded(
        &self,
        stage: CompileStage,
        region: Region,
        value: impl Into<SSAValue>,
    ) -> bool
    where
        V: HasBottom,
    {
        self.fact(stage, region, value)
            .is_some_and(|fact| *fact != V::bottom())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FixpointPhase;

    /// Two-point demand lattice standing in for `kirin_liveness::Live`.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum MiniDemand {
        Dead,
        Live,
    }

    impl Lattice for MiniDemand {
        fn join(&self, other: &Self) -> Self {
            match (self, other) {
                (MiniDemand::Live, _) | (_, MiniDemand::Live) => MiniDemand::Live,
                _ => MiniDemand::Dead,
            }
        }

        fn meet(&self, other: &Self) -> Self {
            match (self, other) {
                (MiniDemand::Dead, _) | (_, MiniDemand::Dead) => MiniDemand::Dead,
                _ => MiniDemand::Live,
            }
        }

        fn is_subseteq(&self, other: &Self) -> bool {
            matches!(
                (self, other),
                (MiniDemand::Dead, _) | (MiniDemand::Live, MiniDemand::Live)
            )
        }
    }

    /// The merge is a real join: it reports the rise exactly once (which is
    /// what schedules the value for reanalysis), then reports stability.
    #[test]
    fn demand_summary_merge_reports_rise_once() {
        let mut summary = DemandSummary(MiniDemand::Dead);
        let rise = summary.merge(
            FixpointPhase::Join,
            DemandSummary(MiniDemand::Live),
            &mut (),
        );
        assert!(rise.is_some());
        assert_eq!(summary.0, MiniDemand::Live);

        let stable = summary.merge(
            FixpointPhase::Join,
            DemandSummary(MiniDemand::Live),
            &mut (),
        );
        assert!(stable.is_none());

        let lower = summary.merge(
            FixpointPhase::Join,
            DemandSummary(MiniDemand::Dead),
            &mut (),
        );
        assert!(
            lower.is_none(),
            "facts only rise: joining bottom is a no-op"
        );
    }
}
