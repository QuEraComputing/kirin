//! The dense backward liveness engine (classic per-program-point liveness).
//!
//! # The analysis
//!
//! A **dense backward** fact is a state per *program point*: for classic
//! liveness, the set of SSA values live at that point. The transfer is the
//! textbook one — kill definitions, gen **all** uses, purity-irrelevant — so
//! the per-point sets carry the conventional (regalloc-grade) meaning. Strong
//! per-point sets are not a separate analysis: intersect these sets with the
//! sparse demand facts
//! ([`SparseBackwardInterpreter`](crate::SparseBackwardInterpreter)).
//!
//! # Layering (owner-summary fixpoint)
//!
//! [`DenseBackwardInterpreter`] is the public engine. Internally it is a
//! wrapper over a [`StandardFixpointInterpreter`] driving a summary-free
//! [`DenseBackwardTransfer`]:
//!
//! - **[`DenseBackwardTransfer`]** is the [`Interp`] delegate: pipeline access,
//!   the real dispatch location, and the *current point state* the dialect
//!   rules transform through [`DenseBackwardInterp::gen_fact`] /
//!   [`DenseBackwardInterp::kill_fact`].
//! - the **[`StandardFixpointInterpreter`]** driver owns the block-boundary
//!   summaries ([`BlockLiveness`], keyed by [`Scoped`] blocks), the block
//!   worklist, and [`BackwardSummaryDeps`] (successor changed → reanalyse
//!   predecessor, registered self-discoveringly by
//!   [`absorb_edges`](DenseBackwardFrameDriver::absorb_edges)).
//!
//! # Owners are blocks; one owner analysis is one backward walk
//!
//! A [`DenseBlockFrame`](crate::DenseBlockFrame) walks the block's statements
//! in reverse, dispatching each through its dialect
//! `Interpretable<I, DenseBackward>` rule. The terminator runs first: its rule
//! gens its own root uses and names its CFG edges
//! ([`DenseBackwardEffect::Edges`]); the driver absorbs them — mapping each
//! successor's converged live-in across the edge (parameter → matching
//! argument, pass-through for non-parameters) — which both seeds the walk
//! state and records the block's `live_out`. Structured dialects push
//! dialect-owned frames ([`DenseBackwardEffect::Push`]) that walk their bodies
//! against the same point state. Per-statement states are not persisted:
//! reconstruct them on demand with
//! [`reconstruct_points`](DenseBackwardInterpreter::reconstruct_points).

use std::marker::PhantomData;

use kirin_ir::{
    Block, CompileStage, HasArguments, HasBottom, HasResults, Lattice, Pipeline, Region, SSAValue,
    StageMeta, Statement,
};

use crate::dense_backward_frames::{DenseBlockFrame, DenseFrameBuild};
use crate::sparse_backward_interp::RegionScope;
use crate::{
    AbstractInterpreter, BackwardSummaryDeps, DenseBackward, DensePointStore, EnvIndex,
    FixpointProfile, Frame, Interp, InterpDispatch, InterpLocation, InterpreterError,
    OwnerSemantics, ProgramPoint, RegionTopology, Scoped, StageQuery, StandardFixpointInterpreter,
    Summary, SummaryDependency, SummaryDependencyIndex, SummaryEffect, query,
};

// ===========================================================================
// Effect + point-state contract + dialect-facing trait
// ===========================================================================

/// One CFG edge as a terminator's dense backward rule states it: `args[i]`
/// flows into the target block's `i`-th parameter.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SuccessorEdge {
    pub target: Block,
    pub args: Vec<SSAValue>,
}

/// What a dense backward rule produced.
pub enum DenseBackwardEffect<F> {
    /// Ordinary statement done; the walk continues with the previous statement.
    Next,
    /// CFG terminator: its successor edges (empty for a function-boundary
    /// terminator like `ret`, whose operand gens the rule already applied).
    Edges(Vec<SuccessorEdge>),
    /// Run a dialect-owned backward frame (structured control flow), then
    /// resume the walk. The rule kills its results and gens its control
    /// operands before pushing; the frame transforms the point state through
    /// the bodies.
    Push { frame: F },
}

/// The point-state contract: a dense backward state is a set of SSA values.
///
/// Implemented by the analysis's state type (e.g. `kirin_liveness::LiveSet`);
/// the join for merge points comes from [`Lattice`] (set union for liveness).
pub trait PointFacts {
    /// Insert `value`; `true` if newly added.
    fn insert(&mut self, value: SSAValue) -> bool;
    /// Remove `value`; `true` if it was present.
    fn remove(&mut self, value: SSAValue) -> bool;
    fn contains(&self, value: SSAValue) -> bool;
    /// The values in the state (order unspecified).
    fn values(&self) -> Vec<SSAValue>;
}

/// Dense-backward engine flavor: point-state access plus
/// [`DenseBackwardEffect`].
///
/// Dense backward rules (`impl Interpretable<I, DenseBackward>`) bound on this
/// trait and transform the *current* point state — the state after the
/// statement on entry to the rule, the state before it on exit.
pub trait DenseBackwardInterp:
    Interp<Kind = DenseBackward, Effect = DenseBackwardEffect<<Self as DenseBackwardInterp>::Frame>>
{
    /// The engine's total backward frame type, carried by
    /// [`DenseBackwardEffect::Push`]. Ordinary dialects never name it.
    type Frame;

    /// Gen: insert `value` into the current point state.
    fn gen_fact(&mut self, value: impl Into<SSAValue>) -> Result<(), Self::Error>;

    /// Kill: remove `value` (a definition) from the current point state.
    fn kill_fact(&mut self, value: impl Into<SSAValue>) -> Result<(), Self::Error>;

    /// The classic (weak) liveness transfer for an ordinary statement: kill
    /// the results, gen **all** operands — purity-irrelevant, so this also
    /// serves calls.
    fn transfer_classic<T>(&mut self, stmt: &T) -> Result<Self::Effect, Self::Error>
    where
        T: for<'a> HasArguments<'a> + for<'a> HasResults<'a>,
    {
        for result in stmt.results() {
            self.kill_fact(*result)?;
        }
        for argument in stmt.arguments() {
            self.gen_fact(*argument)?;
        }
        Ok(DenseBackwardEffect::Next)
    }
}

// ===========================================================================
// DenseBackwardTransfer — the summary-free Interp delegate
// ===========================================================================

/// The summary-free transfer of the dense backward engine: pipeline access,
/// the real dispatch location, and the current point state.
pub struct DenseBackwardTransfer<'ir, S: StageMeta, V, E, F> {
    pipeline: &'ir Pipeline<S>,
    location: Option<InterpLocation>,
    /// The point state the currently walked position sees (the state *after*
    /// the statement when its rule runs).
    state: V,
    activation: EnvIndex,
    #[allow(clippy::type_complexity)]
    _marker: PhantomData<fn() -> (E, F)>,
}

impl<'ir, S: StageMeta, V, E, F> DenseBackwardTransfer<'ir, S, V, E, F>
where
    V: HasBottom,
{
    fn new(pipeline: &'ir Pipeline<S>) -> Self {
        Self {
            pipeline,
            location: None,
            state: V::bottom(),
            activation: EnvIndex::new(0),
            _marker: PhantomData,
        }
    }
}

impl<'ir, S: StageMeta, V, E, F> DenseBackwardTransfer<'ir, S, V, E, F> {
    pub fn pipeline(&self) -> &'ir Pipeline<S> {
        self.pipeline
    }
}

impl<'ir, S, V, E, F> Interp for DenseBackwardTransfer<'ir, S, V, E, F>
where
    S: StageMeta,
    V: Clone,
    E: From<InterpreterError>,
{
    type Value = V;
    type Error = E;
    type Effect = DenseBackwardEffect<F>;
    type Kind = DenseBackward;

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

impl<'ir, S, V, E, F> AbstractInterpreter for DenseBackwardTransfer<'ir, S, V, E, F>
where
    S: StageMeta,
    V: Clone,
    E: From<InterpreterError>,
{
}

impl<'ir, S, V, E, F> DenseBackwardInterp for DenseBackwardTransfer<'ir, S, V, E, F>
where
    S: StageMeta,
    V: Clone + PointFacts,
    E: From<InterpreterError>,
{
    type Frame = F;

    fn gen_fact(&mut self, value: impl Into<SSAValue>) -> Result<(), E> {
        self.state.insert(value.into());
        Ok(())
    }

    fn kill_fact(&mut self, value: impl Into<SSAValue>) -> Result<(), E> {
        self.state.remove(value.into());
        Ok(())
    }
}

// ===========================================================================
// Summary + profile + analysis state
// ===========================================================================

/// One block's converged boundary states. The merge is a real per-half
/// lattice join — `Some(())` iff either boundary rose — which is what drives
/// predecessor rescheduling through [`BackwardSummaryDeps`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockLiveness<V> {
    pub live_in: V,
    pub live_out: V,
}

impl<V: HasBottom> BlockLiveness<V> {
    fn bottom() -> Self {
        Self {
            live_in: V::bottom(),
            live_out: V::bottom(),
        }
    }
}

impl<V> Summary for BlockLiveness<V>
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
        let mut changed = false;
        let live_in = self.live_in.join(&candidate.live_in);
        if live_in != self.live_in {
            self.live_in = live_in;
            changed = true;
        }
        let live_out = self.live_out.join(&candidate.live_out);
        if live_out != self.live_out {
            self.live_out = live_out;
            changed = true;
        }
        changed.then_some(())
    }
}

/// The owner-summary type family for the dense backward engine.
pub struct DenseBackwardProfile<V, E, F> {
    #[allow(clippy::type_complexity)]
    _marker: PhantomData<fn() -> (V, E, F)>,
}

impl<'ir, S, V, E, F> FixpointProfile<DenseBackwardTransfer<'ir, S, V, E, F>>
    for DenseBackwardProfile<V, E, F>
where
    S: StageMeta,
    V: Clone + PartialEq + Lattice,
    E: From<InterpreterError>,
{
    type SummaryKey = Scoped<RegionScope, Block>;
    type Summary = BlockLiveness<V>;
    type Frame = F;
    type Completion = DenseBackwardCompletion<V>;
}

/// Completion payloads of the dense backward frames.
pub enum DenseBackwardCompletion<V> {
    /// A CFG block owner finished its backward walk.
    Block { live_in: V, live_out: V },
    /// A structured body / dialect frame finished; its effects live in the
    /// engine's point state.
    Structured,
}

/// Analysis-local state carried in the driver's `store` slot: the scope,
/// the region topology, and an optional per-point recorder filled by the
/// block frames during [`reconstruct_points`](DenseBackwardInterpreter::reconstruct_points).
pub struct DenseAnalysisState<V> {
    scope: Option<RegionScope>,
    topology: RegionTopology,
    recorder: Option<DensePointStore<V>>,
}

impl<V> Default for DenseAnalysisState<V> {
    fn default() -> Self {
        Self {
            scope: None,
            topology: RegionTopology::default(),
            recorder: None,
        }
    }
}

/// The dense backward driver: a [`StandardFixpointInterpreter`] over
/// [`DenseBackwardTransfer`] with scope-qualified block owners and
/// successor→predecessor dependencies.
pub type DenseBackwardDriver<'ir, S, V, E, F> = StandardFixpointInterpreter<
    DenseBackwardTransfer<'ir, S, V, E, F>,
    DenseBackwardProfile<V, E, F>,
    DenseAnalysisState<V>,
    BackwardSummaryDeps<Scoped<RegionScope, Block>>,
>;

// ===========================================================================
// Driver capabilities (frames run on the driver)
// ===========================================================================

/// The dense-backward frame-driver capability surface: what the dense frames
/// need from the engine. Implemented on the driver (it needs the summaries).
pub trait DenseBackwardFrameDriver:
    Interp<Kind = DenseBackward, Effect = DenseBackwardEffect<Self::Frame>>
{
    /// The engine's total backward frame type.
    type Frame;

    /// Dispatch one statement's dense backward rule (sets the location).
    fn run_statement(
        &mut self,
        stage: CompileStage,
        statement: Statement,
    ) -> Result<Self::Effect, Self::Error>;

    /// A block's statements in program order (terminator, if any, last).
    fn block_statements(&self, block: Block) -> Result<Vec<Statement>, Self::Error>;

    /// The parameters of `block` (structured frames map carried demand).
    fn block_params(&self, stage: CompileStage, block: Block)
    -> Result<Vec<SSAValue>, Self::Error>;

    /// The operands of `block`'s terminator (a structured body's yield slots).
    fn terminator_args(
        &self,
        stage: CompileStage,
        block: Block,
    ) -> Result<Vec<SSAValue>, Self::Error>;

    /// The current point state (cloned).
    fn state(&self) -> Self::Value;

    /// Replace the point state, returning the previous one (used by
    /// structured dialect frames to save/restore around body walks).
    fn replace_state(&mut self, state: Self::Value) -> Self::Value;

    /// Record the current state as the point *before* `statement` (no-op
    /// unless a per-point reconstruction is running).
    fn record_before(&mut self, statement: Statement);

    /// Record the current state as the point *after* `statement` (no-op
    /// unless a per-point reconstruction is running).
    fn record_after(&mut self, statement: Statement);

    /// Absorb a CFG terminator's edges atomically: for each successor, map
    /// its converged live-in across the edge (parameter → matching edge
    /// argument; pass-through for non-parameters — dominated direct
    /// cross-block uses), join the mapped set into the point state, register
    /// the successor→current-owner dependency, and return the joined mapped
    /// set — the block's `live_out`, which deliberately excludes the
    /// terminator's own gens (e.g. the branch condition).
    fn absorb_edges(
        &mut self,
        stage: CompileStage,
        edges: &[SuccessorEdge],
    ) -> Result<Self::Value, Self::Error>;
}

impl<'ir, S, V, E, F> DenseBackwardFrameDriver for DenseBackwardDriver<'ir, S, V, E, F>
where
    S: StageMeta
        + StageQuery
        + InterpDispatch<DenseBackwardTransfer<'ir, S, V, E, F>, DenseBackward>,
    V: Clone + PartialEq + Lattice + HasBottom + PointFacts,
    E: From<InterpreterError>,
{
    type Frame = F;

    fn run_statement(
        &mut self,
        stage: CompileStage,
        statement: Statement,
    ) -> Result<DenseBackwardEffect<F>, E> {
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
        let result = info.dispatch_statement(statement, self.inner_mut());
        self.inner_mut().location = previous;
        result
    }

    fn block_statements(&self, block: Block) -> Result<Vec<Statement>, E> {
        self.store()
            .topology
            .blocks
            .iter()
            .find(|candidate| candidate.block == block)
            .map(|candidate| candidate.stmts.clone())
            .ok_or_else(|| E::from(InterpreterError::MissingBlock(block)))
    }

    fn block_params(&self, stage: CompileStage, block: Block) -> Result<Vec<SSAValue>, E> {
        query::block_params(self.inner().pipeline(), stage, block).map_err(E::from)
    }

    fn terminator_args(&self, stage: CompileStage, block: Block) -> Result<Vec<SSAValue>, E> {
        query::terminator_arguments(self.inner().pipeline(), stage, block).map_err(E::from)
    }

    fn state(&self) -> V {
        self.inner().state.clone()
    }

    fn replace_state(&mut self, state: V) -> V {
        std::mem::replace(&mut self.inner_mut().state, state)
    }

    fn record_before(&mut self, statement: Statement) {
        let state = self.inner().state.clone();
        if let Some(recorder) = self.store_mut().recorder.as_mut() {
            recorder.set(ProgramPoint::Before(statement), state);
        }
    }

    fn record_after(&mut self, statement: Statement) {
        let state = self.inner().state.clone();
        if let Some(recorder) = self.store_mut().recorder.as_mut() {
            recorder.set(ProgramPoint::After(statement), state);
        }
    }

    fn absorb_edges(&mut self, stage: CompileStage, edges: &[SuccessorEdge]) -> Result<V, E> {
        let scope = self
            .store()
            .scope
            .ok_or_else(|| E::from(InterpreterError::Custom("no active backward analysis")))?;

        let mut out = V::bottom();
        for edge in edges {
            let owner = Scoped::new(scope, edge.target);

            // Successor changed → reanalyse the current block.
            if let Some(current) = self.current_owner().cloned() {
                self.dependency_index_mut()
                    .register(&owner, SummaryDependency::Reanalyze(current))
                    .expect("backward dependency index is infallible");
            }

            let Some(summary) = self.summary(&owner) else {
                continue;
            };
            let params = query::block_params(self.inner().pipeline(), stage, edge.target)?;
            let mut mapped = V::bottom();
            for value in summary.live_in.values() {
                match params.iter().position(|param| *param == value) {
                    Some(index) => {
                        if let Some(arg) = edge.args.get(index) {
                            mapped.insert(*arg);
                        }
                    }
                    // A live-in that is not a parameter of the successor is a
                    // dominated direct cross-block use: pass it through.
                    None => {
                        mapped.insert(value);
                    }
                }
            }
            out = out.join(&mapped);
        }

        let state = self.inner().state.join(&out);
        self.inner_mut().state = state;
        Ok(out)
    }
}

// ===========================================================================
// Owner semantics: one block owner = one backward walk
// ===========================================================================

struct DenseBackwardSemantics;

impl<'ir, S, V, E, F>
    OwnerSemantics<
        DenseBackwardDriver<'ir, S, V, E, F>,
        Scoped<RegionScope, Block>,
        BlockLiveness<V>,
        F,
        DenseBackwardCompletion<V>,
        E,
    > for DenseBackwardSemantics
where
    S: StageMeta
        + StageQuery
        + InterpDispatch<DenseBackwardTransfer<'ir, S, V, E, F>, DenseBackward>,
    V: Clone + PartialEq + Lattice + HasBottom + PointFacts,
    E: From<InterpreterError>,
    F: DenseFrameBuild<V, E>,
{
    fn bottom_summary(
        &mut self,
        _interp: &mut DenseBackwardDriver<'ir, S, V, E, F>,
        _owner: &Scoped<RegionScope, Block>,
    ) -> Result<BlockLiveness<V>, E> {
        Ok(BlockLiveness::bottom())
    }

    fn entry_frame(
        &mut self,
        interp: &mut DenseBackwardDriver<'ir, S, V, E, F>,
        owner: &Scoped<RegionScope, Block>,
        _summary: &BlockLiveness<V>,
    ) -> Result<F, E> {
        let (stage, _region) = owner.scope;
        // Each owner walk starts from an empty exit state; the terminator's
        // absorbed edges seed the real live-out.
        interp.replace_state(V::bottom());
        Ok(F::from_block(DenseBlockFrame::cfg_owner(stage, owner.item)))
    }

    fn complete_owner(
        &mut self,
        _interp: &mut DenseBackwardDriver<'ir, S, V, E, F>,
        owner: Scoped<RegionScope, Block>,
        completion: DenseBackwardCompletion<V>,
    ) -> Result<SummaryEffect<Scoped<RegionScope, Block>, BlockLiveness<V>>, E> {
        match completion {
            DenseBackwardCompletion::Block { live_in, live_out } => Ok(SummaryEffect::Update {
                owner,
                candidate: BlockLiveness { live_in, live_out },
            }),
            DenseBackwardCompletion::Structured => Err(E::from(InterpreterError::Custom(
                "a block owner completed with a structured-frame completion",
            ))),
        }
    }
}

// ===========================================================================
// The public engine
// ===========================================================================

/// Dense backward liveness engine (classic per-program-point liveness).
///
/// ```ignore
/// let mut analysis = DenseBackwardInterpreter::<Stage, LiveSet>::new(&pipeline);
/// analysis.analyze(stage, region)?;
/// let boundary = analysis.block_summary(stage, region, block);
/// ```
pub struct DenseBackwardInterpreter<
    'ir,
    S: StageMeta,
    V,
    E = InterpreterError,
    F = crate::StandardDenseBackwardFrame<V, E>,
> where
    V: Clone + PartialEq + Lattice,
    E: From<InterpreterError>,
{
    driver: DenseBackwardDriver<'ir, S, V, E, F>,
}

impl<'ir, S, V, E, F> DenseBackwardInterpreter<'ir, S, V, E, F>
where
    S: StageMeta,
    V: Clone + PartialEq + Lattice + HasBottom,
    E: From<InterpreterError>,
{
    pub fn new(pipeline: &'ir Pipeline<S>) -> Self {
        Self {
            driver: StandardFixpointInterpreter::with_dependency_index(
                DenseBackwardTransfer::new(pipeline),
                DenseAnalysisState::default(),
                (),
                BackwardSummaryDeps::new(),
            ),
        }
    }

    pub fn pipeline(&self) -> &'ir Pipeline<S> {
        self.driver.inner().pipeline()
    }

    /// The converged boundary states of `block` under the `(stage, region)`
    /// scope.
    pub fn block_summary(
        &self,
        stage: CompileStage,
        region: Region,
        block: Block,
    ) -> Option<&BlockLiveness<V>> {
        self.driver.summary(&Scoped::new((stage, region), block))
    }

    /// The analyzed region's CFG blocks (post-`analyze`).
    pub fn cfg_blocks(&self) -> Vec<Block> {
        self.driver
            .store()
            .topology
            .cfg_blocks()
            .map(|block| block.block)
            .collect()
    }
}

impl<'ir, S, V, E, F> DenseBackwardInterpreter<'ir, S, V, E, F>
where
    S: StageMeta
        + StageQuery
        + InterpDispatch<DenseBackwardTransfer<'ir, S, V, E, F>, DenseBackward>,
    V: Clone + PartialEq + Lattice + HasBottom + PointFacts,
    E: From<InterpreterError>,
    F: Frame<DenseBackwardDriver<'ir, S, V, E, F>, Completion = DenseBackwardCompletion<V>>
        + DenseFrameBuild<V, E>,
{
    /// Run the block-boundary fixpoint over `region` in `stage`: seed every
    /// CFG block (a backward analysis must visit them all) and drain the
    /// worklist; dependencies are discovered from the terminators' edges.
    pub fn analyze(&mut self, stage: CompileStage, region: Region) -> Result<(), E> {
        let scope = (stage, region);
        let topology = query::region_topology(self.driver.inner().pipeline(), stage, region)?;
        let owners: Vec<Scoped<RegionScope, Block>> = topology
            .cfg_blocks()
            .map(|block| Scoped::new(scope, block.block))
            .collect();
        *self.driver.store_mut() = DenseAnalysisState {
            scope: Some(scope),
            topology,
            recorder: None,
        };

        let mut semantics = DenseBackwardSemantics;
        self.driver.solve_many(&mut semantics, owners)
    }

    /// Reconstruct every per-statement state — including statements inside
    /// structured bodies, at any nesting depth — by re-walking each converged
    /// CFG block with the recorder enabled. Per-point states are never
    /// persisted by the fixpoint itself; loop bodies record their final
    /// (stable) iteration.
    pub fn reconstruct_points(
        &mut self,
        stage: CompileStage,
        region: Region,
    ) -> Result<DensePointStore<V>, E> {
        let scope = (stage, region);
        self.driver.store_mut().recorder = Some(DensePointStore::new());
        for block in self.cfg_blocks() {
            // The CfgOwner walk re-absorbs the converged successor summaries,
            // so it replays exactly the fixpoint's final states.
            let _ = scope;
            self.driver.replace_state(V::bottom());
            match self
                .driver
                .run_frame(F::from_block(DenseBlockFrame::cfg_owner(stage, block)))?
            {
                DenseBackwardCompletion::Block { .. } => {}
                DenseBackwardCompletion::Structured => {
                    return Err(E::from(InterpreterError::Custom(
                        "a CFG block walk completed as a structured frame",
                    )));
                }
            }
        }
        Ok(self
            .driver
            .store_mut()
            .recorder
            .take()
            .expect("recorder installed above"))
    }
}
