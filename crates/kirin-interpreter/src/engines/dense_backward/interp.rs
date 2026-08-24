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
//!   rules transform through the shape-generic
//!   [`DenseBackwardInterp::point_state_mut`] (liveness rules use the
//!   [`ClassicLivenessInterp`] `gen_live`/`kill_def` spellings, which is where
//!   "a state is a set of values" lives — the engine never assumes it).
//! - the **[`StandardFixpointInterpreter`]** driver owns the block-boundary
//!   summaries ([`BlockLiveness`], keyed by [`Scoped`] blocks), the block
//!   worklist, and [`BackwardSummaryDeps`] (successor changed → reanalyse
//!   predecessor, registered self-discoveringly by
//!   [`absorb_edges`](DenseBackwardFrameEngine::absorb_edges)).
//!
//! # Owners are blocks; one owner analysis is one backward walk
//!
//! A [`DenseBlockFrame`](crate::DenseBlockFrame) walks the block's statements
//! in reverse, dispatching each through its dialect
//! `Interpretable<I, ClassicLiveness>` rule. The terminator runs first: its rule
//! gens its own root uses and names its CFG edges
//! ([`DenseBackwardEffect::Edges`]); the driver absorbs them — mapping each
//! successor's converged live-in across the edge (parameter → matching
//! argument, pass-through for non-parameters) — which both seeds the walk
//! state and records the block's `live_out`. Structured dialects push
//! dialect-owned frames ([`DenseBackwardEffect::Push`]) that walk their bodies
//! against the same point state. Each block walk records statement points and
//! nested structured-block boundaries; later fixpoint iterations overwrite
//! earlier approximations. CFG-owner boundaries remain canonical in their
//! converged summaries. The public fact view merges those disjoint sources
//! into one scope-qualified program-point store.

use std::marker::PhantomData;

use kirin_ir::{
    Block, CompileStage, HasArguments, HasBottom, HasResults, Lattice, Pipeline, SSAValue,
    StageMeta, Statement,
};

use super::frames::DenseBlockFrame;
use crate::Body;
use crate::core::query;
use crate::engines::sparse_backward::BodyScope;
use crate::{
    AbstractInterpreter, BackwardSummaryDeps, ClassicLiveness, DenseBackwardSemantic, EnvIndex,
    FactStore, FixpointProfile, Frame, Interp, InterpDispatch, InterpLocation, InterpreterError,
    OwnerSemantics, ProgramPoint, Scoped, StageQuery, StandardFixpointInterpreter, Summary,
    SummaryDependency, SummaryDependencyIndex, SummaryEffect, TerminatorArgs,
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

/// How a dense backward state moves across a control edge or out of a scope.
///
/// A dense backward state names its facts by [`SSAValue`]. Crossing an edge
/// renames them — the target's parameters become the edge's arguments; leaving
/// a scope drops them. Neither operation says what a fact *is*, which is why
/// this is the only state contract the engine and the dialect frames need:
/// [`Lattice`] merges at join points, this moves facts between vocabularies.
///
/// Pass-through renaming (keep facts the rename does not cover) is the caller's
/// choice, spelled `state.rename(p, a).join(&state.forget(p))` — the CFG edge
/// transfer wants it, a loop back-edge does not.
pub trait DenseBackwardState: Lattice + Sized {
    /// Only the facts named by `params`, renamed to the matching `args`.
    ///
    /// A `params[i]` with no `args[i]` contributes nothing.
    fn rename(&self, params: &[SSAValue], args: &[SSAValue]) -> Self;

    /// Everything except the facts named by `values`.
    fn forget(&self, values: &[SSAValue]) -> Self;
}

/// The point-state contract of [`ClassicLiveness`]: its state is a set of live
/// SSA values.
///
/// This is *semantics*, not shape — it backs
/// [`gen_live`](ClassicLivenessInterp::gen_live) /
/// [`kill_def`](ClassicLivenessInterp::kill_def) and is required by nothing in
/// the engine or the frames. A different dense-backward key brings its own
/// state contract and implements only [`DenseBackwardState`].
pub trait PointFacts {
    /// Insert `value`; `true` if newly added.
    fn insert(&mut self, value: SSAValue) -> bool;
    /// Remove `value`; `true` if it was present.
    fn remove(&mut self, value: SSAValue) -> bool;
    fn contains(&self, value: SSAValue) -> bool;
    /// The values in the state (order unspecified).
    fn values(&self) -> Vec<SSAValue>;
}

/// [`DenseBackwardShape`](crate::DenseBackwardShape)-engine flavor: the
/// *shape-generic* point-state mechanics plus [`DenseBackwardEffect`]. Any
/// dense-backward semantics shares this surface: rules transform the *current*
/// point state — the state after the statement on entry to the rule, the
/// state before it on exit.
///
/// Semantics-specific vocabulary lives in helper traits on top — classic
/// liveness's is [`ClassicLivenessInterp`].
pub trait DenseBackwardInterp:
    Interp<Effect = DenseBackwardEffect<<Self as DenseBackwardInterp>::Frame>>
{
    /// The engine's total backward frame type, carried by
    /// [`DenseBackwardEffect::Push`]. Ordinary dialects never name it.
    type Frame;

    /// The point state being transformed: the state *after* the statement on
    /// entry to a rule, *before* it on exit.
    ///
    /// The engine hands the state over opaquely; how a rule transforms it is
    /// the semantics' business.
    fn point_state(&self) -> &Self::Value;

    /// The point state being transformed, mutably.
    fn point_state_mut(&mut self) -> &mut Self::Value;
}

/// [`ClassicLiveness`]'s helper vocabulary on top of the shape-generic
/// [`DenseBackwardInterp`]: gen/kill are liveness's names for the point-state
/// mutations, and the classic kill-defs/gen-all-uses transfer is the
/// ordinary-dialect one-liner. Classic-liveness rules
/// (`impl Interpretable<I, ClassicLiveness>`) bound on this trait.
///
/// Pinned to `Semantics = ClassicLiveness` via the supertrait (rustc
/// elaborates supertraits, so rules bounding `I: ClassicLivenessInterp` need
/// no extra clauses), and blanket-implemented for every classic-liveness
/// dense-backward engine. [`PointFacts`] rides in the same supertrait as an
/// associated-type bound for the same reason: elaboration means liveness rules
/// inherit it and never spell it.
pub trait ClassicLivenessInterp:
    DenseBackwardInterp + Interp<Semantics = ClassicLiveness, Value: PointFacts>
{
    /// Gen: mark `value` live at the current point (a use).
    fn gen_live(&mut self, value: impl Into<SSAValue>) -> Result<(), Self::Error> {
        self.point_state_mut().insert(value.into());
        Ok(())
    }

    /// Kill: remove `value` (a definition) from the current point state.
    fn kill_def(&mut self, value: impl Into<SSAValue>) -> Result<(), Self::Error> {
        self.point_state_mut().remove(value.into());
        Ok(())
    }

    /// The classic (weak) liveness transfer for an ordinary statement: kill
    /// the results, gen **all** operands — purity-irrelevant, so this also
    /// serves calls.
    fn gen_uses_kill_defs<T>(&mut self, stmt: &T) -> Result<Self::Effect, Self::Error>
    where
        T: for<'a> HasArguments<'a> + for<'a> HasResults<'a>,
    {
        for result in stmt.results() {
            self.kill_def(*result)?;
        }
        for argument in stmt.arguments() {
            self.gen_live(*argument)?;
        }
        Ok(DenseBackwardEffect::Next)
    }
}

impl<I> ClassicLivenessInterp for I
where
    I: DenseBackwardInterp + Interp<Semantics = ClassicLiveness>,
    I::Value: PointFacts,
{
}

// ===========================================================================
// DenseBackwardTransfer — the summary-free Interp delegate
// ===========================================================================

/// The summary-free transfer of the dense backward engine: pipeline access,
/// the real dispatch location, and the current point state.
pub struct DenseBackwardTransfer<'ir, S: StageMeta, V, E, F, Sem = ClassicLiveness> {
    pipeline: &'ir Pipeline<S>,
    location: Option<InterpLocation>,
    /// The point state the currently walked position sees (the state *after*
    /// the statement when its rule runs).
    state: V,
    activation: EnvIndex,
    #[allow(clippy::type_complexity)]
    _marker: PhantomData<fn() -> (E, F, Sem)>,
}

impl<'ir, S: StageMeta, V, E, F, Sem> DenseBackwardTransfer<'ir, S, V, E, F, Sem>
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

impl<'ir, S: StageMeta, V, E, F, Sem> DenseBackwardTransfer<'ir, S, V, E, F, Sem> {
    pub fn pipeline(&self) -> &'ir Pipeline<S> {
        self.pipeline
    }
}

impl<'ir, S, V, E, F, Sem> Interp for DenseBackwardTransfer<'ir, S, V, E, F, Sem>
where
    S: StageMeta,
    V: Clone,
    E: From<InterpreterError>,
    Sem: DenseBackwardSemantic,
{
    type Value = V;
    type Error = E;
    type Effect = DenseBackwardEffect<F>;
    type Semantics = Sem;

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

impl<'ir, S, V, E, F, Sem> AbstractInterpreter for DenseBackwardTransfer<'ir, S, V, E, F, Sem>
where
    S: StageMeta,
    V: Clone,
    E: From<InterpreterError>,
    Sem: DenseBackwardSemantic,
{
}

impl<'ir, S, V, E, F, Sem> DenseBackwardInterp for DenseBackwardTransfer<'ir, S, V, E, F, Sem>
where
    S: StageMeta,
    V: Clone,
    E: From<InterpreterError>,
    Sem: DenseBackwardSemantic,
{
    type Frame = F;

    fn point_state(&self) -> &V {
        &self.state
    }

    fn point_state_mut(&mut self) -> &mut V {
        &mut self.state
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

impl<'ir, S, V, E, F, Sem> FixpointProfile<DenseBackwardTransfer<'ir, S, V, E, F, Sem>>
    for DenseBackwardProfile<V, E, F>
where
    S: StageMeta,
    V: Clone + PartialEq + Lattice,
    E: From<InterpreterError>,
    Sem: DenseBackwardSemantic,
{
    type SummaryKey = Scoped<BodyScope, Block>;
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

/// The dense backward driver: a [`StandardFixpointInterpreter`] over
/// [`DenseBackwardTransfer`] with scope-qualified block owners and
/// successor→predecessor dependencies.
pub type DenseBackwardDriver<'ir, S, V, E, F, Sem = ClassicLiveness> = StandardFixpointInterpreter<
    DenseBackwardTransfer<'ir, S, V, E, F, Sem>,
    DenseBackwardProfile<V, E, F>,
    FactStore<Scoped<BodyScope, ProgramPoint>, V>,
    BackwardSummaryDeps<Scoped<BodyScope, Block>>,
>;

// ===========================================================================
// Driver capabilities (frames run on the driver)
// ===========================================================================

/// The dense-backward engine-capability surface: what the dense frames
/// need from the engine. Implemented on the driver (it needs the summaries).
pub trait DenseBackwardFrameEngine: Interp<Effect = DenseBackwardEffect<Self::Frame>> {
    /// The engine's total backward frame type.
    type Frame;

    /// Dispatch one statement's dense backward rule (sets the location).
    fn run_statement(
        &mut self,
        stage: CompileStage,
        statement: Statement,
    ) -> Result<Self::Effect, Self::Error>;

    /// The last logical statement of a block (the terminator when present).
    fn last_statement(
        &self,
        stage: CompileStage,
        block: Block,
    ) -> Result<Option<Statement>, Self::Error>;

    /// The statement immediately before `before` in the block's logical
    /// statement order.
    fn previous_statement(
        &self,
        stage: CompileStage,
        block: Block,
        before: Statement,
    ) -> Result<Option<Statement>, Self::Error>;

    /// The parameters of `block` (structured frames map carried demand).
    fn block_params(&self, stage: CompileStage, block: Block)
    -> Result<Vec<SSAValue>, Self::Error>;

    /// The operands of `block`'s terminator (a structured body's yield slots).
    fn terminator_args(
        &self,
        stage: CompileStage,
        block: Block,
    ) -> Result<TerminatorArgs, Self::Error>;

    /// The current point state (cloned).
    fn state(&self) -> Self::Value;

    /// Replace the point state, returning the previous one (used by
    /// structured dialect frames to save/restore around body walks).
    fn replace_state(&mut self, state: Self::Value) -> Self::Value;

    /// Store `facts` at a block or statement program point, overwriting the
    /// approximation recorded by any earlier fixpoint iteration.
    fn record_point(&mut self, point: ProgramPoint, facts: Self::Value);

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

impl<'ir, S, V, E, F, Sem> DenseBackwardFrameEngine for DenseBackwardDriver<'ir, S, V, E, F, Sem>
where
    S: StageMeta + StageQuery + InterpDispatch<DenseBackwardTransfer<'ir, S, V, E, F, Sem>>,
    V: Clone + PartialEq + Lattice + HasBottom + DenseBackwardState,
    E: From<InterpreterError>,
    Sem: DenseBackwardSemantic,
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

    fn last_statement(&self, stage: CompileStage, block: Block) -> Result<Option<Statement>, E> {
        query::last_statement(self.inner().pipeline(), stage, block).map_err(E::from)
    }

    fn previous_statement(
        &self,
        stage: CompileStage,
        block: Block,
        before: Statement,
    ) -> Result<Option<Statement>, E> {
        query::previous_statement(self.inner().pipeline(), stage, block, before).map_err(E::from)
    }

    fn block_params(&self, stage: CompileStage, block: Block) -> Result<Vec<SSAValue>, E> {
        query::block_params(self.inner().pipeline(), stage, block).map_err(E::from)
    }

    fn terminator_args(&self, stage: CompileStage, block: Block) -> Result<TerminatorArgs, E> {
        query::terminator_arguments(self.inner().pipeline(), stage, block).map_err(E::from)
    }

    fn state(&self) -> V {
        self.inner().state.clone()
    }

    fn replace_state(&mut self, state: V) -> V {
        std::mem::replace(&mut self.inner_mut().state, state)
    }

    fn record_point(&mut self, point: ProgramPoint, facts: V) {
        let scope = self
            .current_owner()
            .map(|owner| owner.scope)
            .expect("dense frames only run while analyzing an owner");
        self.store_mut().set(Scoped::new(scope, point), facts);
    }

    fn absorb_edges(&mut self, stage: CompileStage, edges: &[SuccessorEdge]) -> Result<V, E> {
        let current = self
            .current_owner()
            .cloned()
            .ok_or_else(|| E::from(InterpreterError::Custom("no active backward owner")))?;
        let scope = current.scope;

        let mut out = V::bottom();
        for edge in edges {
            let owner = Scoped::new(scope, edge.target);

            // Successor changed → reanalyse the current block.
            self.dependency_index_mut()
                .register(&owner, SummaryDependency::Reanalyze(current.clone()))
                .expect("backward dependency index is infallible");

            let Some(summary) = self.summary(&owner) else {
                continue;
            };
            let params = query::block_params(self.inner().pipeline(), stage, edge.target)?;
            // The successor's entry state in this block's vocabulary: its
            // parameters renamed to the edge's arguments, joined with the
            // facts the rename does not cover — live-ins that are not
            // parameters are dominated direct cross-block uses and pass
            // through unchanged.
            let entry = &summary.live_in;
            let mapped = entry
                .rename(&params, &edge.args)
                .join(&entry.forget(&params));
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

impl<'ir, S, V, E, F, Sem>
    OwnerSemantics<
        DenseBackwardDriver<'ir, S, V, E, F, Sem>,
        Scoped<BodyScope, Block>,
        BlockLiveness<V>,
        F,
        DenseBackwardCompletion<V>,
        E,
    > for DenseBackwardSemantics
where
    S: StageMeta + StageQuery + InterpDispatch<DenseBackwardTransfer<'ir, S, V, E, F, Sem>>,
    V: Clone + PartialEq + Lattice + HasBottom + DenseBackwardState,
    E: From<InterpreterError>,
    Sem: DenseBackwardSemantic,
    F: From<DenseBlockFrame<V, E>>,
{
    fn bottom_summary(
        &mut self,
        _interp: &mut DenseBackwardDriver<'ir, S, V, E, F, Sem>,
        _owner: &Scoped<BodyScope, Block>,
    ) -> Result<BlockLiveness<V>, E> {
        Ok(BlockLiveness::bottom())
    }

    fn entry_frame(
        &mut self,
        interp: &mut DenseBackwardDriver<'ir, S, V, E, F, Sem>,
        owner: &Scoped<BodyScope, Block>,
        _summary: &BlockLiveness<V>,
    ) -> Result<F, E> {
        let (stage, _cfg) = owner.scope;
        // Each owner walk starts from an empty exit state; the terminator's
        // absorbed edges seed the real live-out.
        interp.replace_state(V::bottom());
        Ok(DenseBlockFrame::cfg_owner(stage, owner.item).into())
    }

    fn complete_owner(
        &mut self,
        _interp: &mut DenseBackwardDriver<'ir, S, V, E, F, Sem>,
        owner: Scoped<BodyScope, Block>,
        completion: DenseBackwardCompletion<V>,
    ) -> Result<SummaryEffect<Scoped<BodyScope, Block>, BlockLiveness<V>>, E> {
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
/// analysis.analyze(stage, cfg)?;
/// let point = Scoped::new((stage, Body::CFG(cfg)), ProgramPoint::BlockEntry(block));
/// let live_in = analysis.point_facts(point);
/// ```
pub struct DenseBackwardInterpreter<
    'ir,
    S: StageMeta,
    V,
    E = InterpreterError,
    F = DenseBlockFrame<V, E>,
    Sem = ClassicLiveness,
> where
    V: Clone + PartialEq + Lattice,
    E: From<InterpreterError>,
    Sem: DenseBackwardSemantic,
    Sem: DenseBackwardSemantic,
{
    driver: DenseBackwardDriver<'ir, S, V, E, F, Sem>,
}

impl<'ir, S, V, E, F, Sem> DenseBackwardInterpreter<'ir, S, V, E, F, Sem>
where
    S: StageMeta,
    V: Clone + PartialEq + Lattice + HasBottom,
    E: From<InterpreterError>,
    Sem: DenseBackwardSemantic,
{
    pub fn new(pipeline: &'ir Pipeline<S>) -> Self {
        Self {
            driver: StandardFixpointInterpreter::with_dependency_index(
                DenseBackwardTransfer::new(pipeline),
                FactStore::new(),
                (),
                BackwardSummaryDeps::new(),
            ),
        }
    }

    pub fn pipeline(&self) -> &'ir Pipeline<S> {
        self.driver.inner().pipeline()
    }

    /// The converged fact at a scope-qualified program point.
    ///
    /// CFG-owner boundaries come directly from their fixpoint summaries;
    /// statement and nested structured-block points come from the point store.
    /// Each fact therefore has one mutable source during solving.
    pub fn point_facts(&self, point: Scoped<BodyScope, ProgramPoint>) -> Option<&V> {
        let summary_fact = match point.item {
            ProgramPoint::BlockEntry(block) => self
                .driver
                .summary(&Scoped::new(point.scope, block))
                .map(|summary| &summary.live_in),
            ProgramPoint::BlockExit(block) => self
                .driver
                .summary(&Scoped::new(point.scope, block))
                .map(|summary| &summary.live_out),
            ProgramPoint::Before(_) | ProgramPoint::After(_) => None,
        };
        summary_fact.or_else(|| self.driver.store().get(point))
    }

    /// Snapshot the active analysis as one scope-qualified program-point fact
    /// store.
    ///
    /// The solver keeps CFG-owner boundaries in summaries because they drive
    /// convergence. This copies each final boundary into the returned result;
    /// it does not create a second mutable representation inside the engine.
    pub fn facts(&self) -> FactStore<Scoped<BodyScope, ProgramPoint>, V> {
        let mut facts = self.driver.store().clone();

        for (owner, summary) in self.driver.summaries() {
            facts.set(
                Scoped::new(owner.scope, ProgramPoint::BlockEntry(owner.item)),
                summary.live_in.clone(),
            );
            facts.set(
                Scoped::new(owner.scope, ProgramPoint::BlockExit(owner.item)),
                summary.live_out.clone(),
            );
        }
        facts
    }
}

impl<'ir, S, V, E, F, Sem> DenseBackwardInterpreter<'ir, S, V, E, F, Sem>
where
    S: StageMeta + StageQuery + InterpDispatch<DenseBackwardTransfer<'ir, S, V, E, F, Sem>>,
    V: Clone + PartialEq + Lattice + HasBottom + DenseBackwardState,
    E: From<InterpreterError>,
    Sem: DenseBackwardSemantic,
    F: Frame<DenseBackwardDriver<'ir, S, V, E, F, Sem>, F, Completion = DenseBackwardCompletion<V>>
        + From<DenseBlockFrame<V, E>>,
{
    /// The blocks directly selected as fixpoint owners for `body`.
    ///
    /// This reads the current IR rather than returning cached analysis state.
    fn direct_body_blocks(
        &self,
        stage: CompileStage,
        body: impl Into<Body>,
    ) -> Result<Vec<Block>, E> {
        query::direct_body_blocks(self.driver.inner().pipeline(), stage, body.into())
            .map_err(E::from)
    }

    /// Run the block-boundary fixpoint over `cfg` in `stage`: seed every
    /// CFG block (a backward analysis must visit them all) and drain the
    /// worklist; dependencies are discovered from the terminators' edges.
    pub fn analyze(&mut self, stage: CompileStage, body: impl Into<Body>) -> Result<(), E> {
        let body = body.into();
        let scope = (stage, body);
        let blocks = self.direct_body_blocks(stage, body)?;
        let owners: Vec<Scoped<BodyScope, Block>> = blocks
            .iter()
            .copied()
            .map(|block| Scoped::new(scope, block))
            .collect();
        let pipeline = self.driver.inner().pipeline();
        self.driver = Self::new(pipeline).driver;

        let mut semantics = DenseBackwardSemantics;
        self.driver.solve_many(&mut semantics, owners)
    }
}
