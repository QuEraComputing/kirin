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
//! # Layering
//!
//! [`DenseBackwardInterpreter`] is the public engine. Internally it is a
//! wrapper over a [`StandardFixpointInterpreter`] driving a
//! [`DenseBackwardTransfer`]:
//!
//! - **[`DenseBackwardTransfer`]** is the [`Interp`] delegate: pipeline access,
//!   the point facts (one [`EnvStore`] environment for the analyzed callable),
//!   the real dispatch location, and the *current point state* the dialect
//!   rules transform through the shape-generic
//!   [`DenseBackwardInterp::insert_fact`] / [`DenseBackwardInterp::remove_fact`]
//!   (liveness rules use the [`ClassicLivenessInterp`] `gen_live`/`kill_def`
//!   spellings).
//! - the **[`StandardFixpointInterpreter`]** driver owns the block worklist
//!   and the readers of each block's entry point ([`DenseReaders`],
//!   registered by [`absorb_edges`](DenseBackwardFrameEngine::absorb_edges)).
//!
//! # Work is a block walk; facts are program points
//!
//! A work item is `(EnvIndex, Body::Block(block))`. A
//! [`DenseBlockFrame`](crate::DenseBlockFrame) walks the block's statements
//! in reverse, dispatching each through its dialect
//! `Interpretable<I, ClassicLiveness>` rule. The terminator runs first: its rule
//! gens its own root uses and names its CFG edges
//! ([`DenseBackwardEffect::Edges`]); the driver absorbs them — mapping each
//! successor's live-in across the edge (parameter → matching argument,
//! pass-through for non-parameters) — which both seeds the walk state and
//! yields the block's `live_out`. Structured dialects push dialect-owned
//! frames ([`DenseBackwardEffect::Push`]) that walk their bodies against the
//! same point state. Every recorded point — statement points, nested
//! structured-block boundaries, and the walked block's own boundaries — joins
//! into the environment through [`EnvStore::join`], so a block's
//! `BlockEntry` always equals its first statement's `Before`. When a block's
//! `BlockEntry` rises, the driver rewalks the predecessors that read it.

use std::collections::HashMap;
use std::marker::PhantomData;

use kirin_ir::{
    Block, CompileStage, HasArguments, HasBottom, HasResults, Lattice, Pipeline, SSAValue,
    StageMeta, Statement, Symbol,
};

use super::frames::DenseBlockFrame;
use crate::Body;
use crate::core::query;
use crate::{
    AbstractInterpreter, BlockQueries, Callee, ClassicLiveness, DenseBackwardSemantic, EnvIndex,
    EnvStore, FactStore, FixpointProfile, Frame, Interp, InterpDispatch, InterpLocation,
    InterpreterError, LinkTarget, Linker, ProgramPoint, SameStageLinker, StageQuery,
    StandardFixpointInterpreter, StatementDispatch, TerminatorArgs, WorkAnalysis,
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

    /// Insert `value` into the current point state.
    fn insert_fact(&mut self, value: impl Into<SSAValue>) -> Result<(), Self::Error>;

    /// Remove `value` from the current point state.
    fn remove_fact(&mut self, value: impl Into<SSAValue>) -> Result<(), Self::Error>;
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
/// dense-backward engine.
pub trait ClassicLivenessInterp: DenseBackwardInterp + Interp<Semantics = ClassicLiveness> {
    /// Gen: mark `value` live at the current point (a use).
    fn gen_live(&mut self, value: impl Into<SSAValue>) -> Result<(), Self::Error> {
        self.insert_fact(value)
    }

    /// Kill: remove `value` (a definition) from the current point state.
    fn kill_def(&mut self, value: impl Into<SSAValue>) -> Result<(), Self::Error> {
        self.remove_fact(value)
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

impl<I> ClassicLivenessInterp for I where
    I: DenseBackwardInterp + Interp<Semantics = ClassicLiveness>
{
}

// ===========================================================================
// DenseBackwardTransfer — point facts, dispatch location, and walk state
// ===========================================================================

/// The transfer of the dense backward engine: pipeline access, the point
/// facts of the analyzed callable, the real dispatch location, and the
/// current point state.
pub struct DenseBackwardTransfer<'ir, S: StageMeta, V, E, F, Sem = ClassicLiveness> {
    pipeline: &'ir Pipeline<S>,
    location: Option<InterpLocation>,
    /// Point facts, one environment for the analyzed callable.
    env: EnvStore<LinkTarget, ProgramPoint, V>,
    /// The analyzed callable and its environment.
    root: Option<(LinkTarget, EnvIndex)>,
    /// The point state the currently walked position sees (the state *after*
    /// the statement when its rule runs). Kill and gen act on it; only
    /// recorded points reach the store.
    state: V,
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
            env: EnvStore::new(),
            root: None,
            state: V::bottom(),
            _marker: PhantomData,
        }
    }
}

impl<'ir, S: StageMeta, V, E, F, Sem> DenseBackwardTransfer<'ir, S, V, E, F, Sem> {
    pub fn pipeline(&self) -> &'ir Pipeline<S> {
        self.pipeline
    }

    /// The analyzed callable and its environment.
    fn root(&self) -> Result<(LinkTarget, EnvIndex), InterpreterError> {
        self.root.ok_or(InterpreterError::Custom(
            "no dense backward analysis is active",
        ))
    }

    /// Join `facts` into `point` and report whether it rose. A recorded point
    /// stays present even at bottom, so results tell "nothing live" from
    /// "never reached".
    fn record(
        &mut self,
        env: EnvIndex,
        point: ProgramPoint,
        facts: &V,
    ) -> Result<bool, InterpreterError>
    where
        V: Lattice + HasBottom + Clone,
    {
        let changed = self.env.join(env, point, facts)?;
        if !changed && !self.env.environment(env)?.contains(point) {
            self.env.write(env, point, facts.clone())?;
        }
        Ok(changed)
    }
}

impl<'ir, S, V, E, F, Sem> DenseBackwardTransfer<'ir, S, V, E, F, Sem>
where
    S: StageMeta + InterpDispatch<Self>,
    V: Clone,
    E: From<InterpreterError>,
    Sem: DenseBackwardSemantic,
{
    /// Dispatch one statement's dense backward rule at `stage` in environment
    /// `index`, as the sparse transfers do.
    fn run_statement(
        &mut self,
        stage: CompileStage,
        statement: Statement,
        index: EnvIndex,
    ) -> Result<DenseBackwardEffect<F>, E> {
        let info = self
            .pipeline
            .stage(stage)
            .ok_or_else(|| E::from(InterpreterError::MissingStage(stage)))?;
        let previous = self.location.replace(InterpLocation {
            stage,
            statement,
            index,
        });
        let result = info.dispatch_statement(statement, self);
        self.location = previous;
        result
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
        self.location.expect("interp location not set").index
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
    V: Clone + PointFacts,
    E: From<InterpreterError>,
    Sem: DenseBackwardSemantic,
{
    type Frame = F;

    fn insert_fact(&mut self, value: impl Into<SSAValue>) -> Result<(), E> {
        self.state.insert(value.into());
        Ok(())
    }

    fn remove_fact(&mut self, value: impl Into<SSAValue>) -> Result<(), E> {
        self.state.remove(value.into());
        Ok(())
    }
}

// ===========================================================================
// Work items + profile + driver
// ===========================================================================

/// One dense work item: walk `Body` (a CFG block, as [`Body::Block`]) in the
/// environment `EnvIndex`.
pub(super) type DenseWork = (EnvIndex, Body);

/// The driver's `store` slot: the blocks to rewalk when a block's entry point
/// rises, registered when a predecessor's walk reads it.
pub type DenseReaders = HashMap<(EnvIndex, ProgramPoint), Vec<DenseWork>>;

/// The work-item type family for the dense backward engine. Facts live in the
/// transfer's [`EnvStore`].
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
    type Work = DenseWork;
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
/// [`DenseBackwardTransfer`] whose work items are block walks, with the
/// successor → predecessor readers in its `store` slot.
pub type DenseBackwardDriver<'ir, S, V, E, F, Sem = ClassicLiveness> = StandardFixpointInterpreter<
    DenseBackwardTransfer<'ir, S, V, E, F, Sem>,
    DenseBackwardProfile<V, E, F>,
    DenseReaders,
>;

// ===========================================================================
// Driver capabilities (frames run on the driver)
// ===========================================================================

/// The dense-backward engine-capability surface: what the dense frames
/// need from the engine, on top of the shared [`StatementDispatch`] and
/// [`BlockQueries`] capabilities. Implemented on the driver (it registers
/// readers).
pub trait DenseBackwardFrameEngine:
    StatementDispatch + BlockQueries + Interp<Effect = DenseBackwardEffect<Self::Frame>>
{
    /// The engine's total backward frame type.
    type Frame;

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

    /// Join `facts` into a block or statement program point. Each walk
    /// records its points again; the join keeps every point consistent with
    /// the block boundaries around it.
    fn record_point(&mut self, point: ProgramPoint, facts: Self::Value) -> Result<(), Self::Error>;

    /// Absorb a CFG terminator's edges atomically: for each successor, map
    /// its converged live-in across the edge (parameter → matching edge
    /// argument; pass-through for non-parameters — dominated direct
    /// cross-block uses), join the mapped set into the point state, register
    /// the current block as a reader of the successor's entry, and return the
    /// joined mapped set — the block's `live_out`, which deliberately
    /// excludes the terminator's own gens (e.g. the branch condition).
    fn absorb_edges(
        &mut self,
        stage: CompileStage,
        edges: &[SuccessorEdge],
    ) -> Result<Self::Value, Self::Error>;
}

impl<'ir, S, V, E, F, Sem> DenseBackwardDriver<'ir, S, V, E, F, Sem>
where
    S: StageMeta,
    V: Clone + PartialEq + Lattice + HasBottom,
    E: From<InterpreterError>,
    Sem: DenseBackwardSemantic,
{
    /// The work item being walked.
    fn active_work(&self) -> Result<DenseWork, E> {
        self.current_work()
            .copied()
            .ok_or_else(|| E::from(InterpreterError::Custom("no active backward work")))
    }
}

impl<'ir, S, V, E, F, Sem> StatementDispatch for DenseBackwardDriver<'ir, S, V, E, F, Sem>
where
    S: StageMeta + StageQuery + InterpDispatch<DenseBackwardTransfer<'ir, S, V, E, F, Sem>>,
    V: Clone + PartialEq + Lattice + HasBottom,
    E: From<InterpreterError>,
    Sem: DenseBackwardSemantic,
{
    fn run_statement(
        &mut self,
        stage: CompileStage,
        statement: Statement,
        index: EnvIndex,
    ) -> Result<DenseBackwardEffect<F>, E> {
        self.inner_mut().run_statement(stage, statement, index)
    }
}

impl<'ir, S, V, E, F, Sem> BlockQueries for DenseBackwardDriver<'ir, S, V, E, F, Sem>
where
    S: StageMeta + StageQuery,
    V: Clone + PartialEq + Lattice + HasBottom,
    E: From<InterpreterError>,
    Sem: DenseBackwardSemantic,
{
    fn block_params(&self, stage: CompileStage, block: Block) -> Result<Vec<SSAValue>, E> {
        query::block_params(self.inner().pipeline(), stage, block).map_err(E::from)
    }

    fn first_statement(&self, stage: CompileStage, block: Block) -> Result<Option<Statement>, E> {
        query::first_statement(self.inner().pipeline(), stage, block).map_err(E::from)
    }

    fn next_statement(
        &self,
        stage: CompileStage,
        block: Block,
        after: Statement,
    ) -> Result<Option<Statement>, E> {
        query::next_statement(self.inner().pipeline(), stage, block, after).map_err(E::from)
    }
}

impl<'ir, S, V, E, F, Sem> DenseBackwardFrameEngine for DenseBackwardDriver<'ir, S, V, E, F, Sem>
where
    S: StageMeta + StageQuery + InterpDispatch<DenseBackwardTransfer<'ir, S, V, E, F, Sem>>,
    V: Clone + PartialEq + Lattice + HasBottom + PointFacts,
    E: From<InterpreterError>,
    Sem: DenseBackwardSemantic,
{
    type Frame = F;

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

    fn terminator_args(&self, stage: CompileStage, block: Block) -> Result<TerminatorArgs, E> {
        query::terminator_arguments(self.inner().pipeline(), stage, block).map_err(E::from)
    }

    fn state(&self) -> V {
        self.inner().state.clone()
    }

    fn replace_state(&mut self, state: V) -> V {
        std::mem::replace(&mut self.inner_mut().state, state)
    }

    fn record_point(&mut self, point: ProgramPoint, facts: V) -> Result<(), E> {
        let (env, _) = self.active_work()?;
        self.inner_mut().record(env, point, &facts)?;
        Ok(())
    }

    fn absorb_edges(&mut self, stage: CompileStage, edges: &[SuccessorEdge]) -> Result<V, E> {
        let current = self.active_work()?;
        let env = current.0;

        let mut out = V::bottom();
        for edge in edges {
            let entry = ProgramPoint::BlockEntry(edge.target);

            // Successor entry rose → rewalk the current block.
            let readers = self.store_mut().entry((env, entry)).or_default();
            if !readers.contains(&current) {
                readers.push(current);
            }

            let Some(live_in) = self.inner().env.read(env, entry)? else {
                continue;
            };
            let params = query::block_params(self.inner().pipeline(), stage, edge.target)?;
            let mut mapped = V::bottom();
            for value in live_in.values() {
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
// Work analysis: one work item = one block walk
// ===========================================================================

struct DenseBackwardWorkAnalysis;

const NOT_A_BLOCK: InterpreterError =
    InterpreterError::Custom("dense backward work must be a CFG block");

impl<'ir, S, V, E, F, Sem>
    WorkAnalysis<
        DenseBackwardDriver<'ir, S, V, E, F, Sem>,
        DenseWork,
        F,
        DenseBackwardCompletion<V>,
        E,
    > for DenseBackwardWorkAnalysis
where
    S: StageMeta + StageQuery + InterpDispatch<DenseBackwardTransfer<'ir, S, V, E, F, Sem>>,
    V: Clone + PartialEq + Lattice + HasBottom + PointFacts,
    E: From<InterpreterError>,
    Sem: DenseBackwardSemantic,
    F: From<DenseBlockFrame<V, E>>,
{
    fn entry_frame(
        &mut self,
        interp: &mut DenseBackwardDriver<'ir, S, V, E, F, Sem>,
        &(env, body): &DenseWork,
    ) -> Result<F, E> {
        let Body::Block(block) = body else {
            return Err(E::from(NOT_A_BLOCK));
        };
        let (target, _) = interp.inner().root()?;
        // Each walk starts from an empty exit state; the terminator's
        // absorbed edges seed the real live-out.
        interp.replace_state(V::bottom());
        Ok(DenseBlockFrame::cfg_owner(target.stage, env, block).into())
    }

    fn complete_work(
        &mut self,
        interp: &mut DenseBackwardDriver<'ir, S, V, E, F, Sem>,
        (env, body): DenseWork,
        completion: DenseBackwardCompletion<V>,
    ) -> Result<(), E> {
        let Body::Block(block) = body else {
            return Err(E::from(NOT_A_BLOCK));
        };
        let DenseBackwardCompletion::Block { live_in, live_out } = completion else {
            return Err(E::from(InterpreterError::Custom(
                "a block owner completed with a structured-frame completion",
            )));
        };
        let entry = ProgramPoint::BlockEntry(block);
        interp
            .inner_mut()
            .record(env, ProgramPoint::BlockExit(block), &live_out)?;
        if interp.inner_mut().record(env, entry, &live_in)? {
            let readers = interp
                .store()
                .get(&(env, entry))
                .cloned()
                .unwrap_or_default();
            for reader in readers {
                interp.schedule(reader);
            }
        }
        Ok(())
    }
}

// ===========================================================================
// The public engine
// ===========================================================================

/// Dense backward liveness engine (classic per-program-point liveness).
///
/// ```ignore
/// let analysis = DenseBackwardInterpreter::<Stage, LiveSet>::new(&pipeline);
/// let (_target, analysis) = analysis.analyze(stage, callee)?;
/// let live_in = analysis.point_facts(ProgramPoint::BlockEntry(block));
/// ```
pub struct DenseBackwardInterpreter<
    'ir,
    S: StageMeta,
    V,
    E = InterpreterError,
    F = DenseBlockFrame<V, E>,
    Lk = SameStageLinker,
    Sem = ClassicLiveness,
> where
    V: Clone + PartialEq + Lattice,
    E: From<InterpreterError>,
    Sem: DenseBackwardSemantic,
    Sem: DenseBackwardSemantic,
{
    driver: DenseBackwardDriver<'ir, S, V, E, F, Sem>,
    linker: Lk,
}

impl<'ir, S, V, E, F, Sem> DenseBackwardInterpreter<'ir, S, V, E, F, SameStageLinker, Sem>
where
    S: StageMeta,
    V: Clone + PartialEq + Lattice + HasBottom,
    E: From<InterpreterError>,
    Sem: DenseBackwardSemantic,
{
    pub fn new(pipeline: &'ir Pipeline<S>) -> Self {
        Self {
            driver: StandardFixpointInterpreter::new(
                DenseBackwardTransfer::new(pipeline),
                DenseReaders::new(),
            ),
            linker: SameStageLinker,
        }
    }
}

impl<'ir, S, V, E, F, Lk, Sem> DenseBackwardInterpreter<'ir, S, V, E, F, Lk, Sem>
where
    S: StageMeta,
    V: Clone + PartialEq + Lattice + HasBottom,
    E: From<InterpreterError>,
    Sem: DenseBackwardSemantic,
{
    /// Replace the calling convention used by the canonical callable root.
    pub fn with_linker<Lk2>(
        self,
        linker: Lk2,
    ) -> DenseBackwardInterpreter<'ir, S, V, E, F, Lk2, Sem> {
        DenseBackwardInterpreter {
            driver: self.driver,
            linker,
        }
    }

    pub fn pipeline(&self) -> &'ir Pipeline<S> {
        self.driver.inner().pipeline()
    }

    /// The converged fact at a program point of the analyzed callable.
    pub fn point_facts(&self, point: ProgramPoint) -> Option<&V> {
        let transfer = self.driver.inner();
        let (_, env) = transfer.root().ok()?;
        transfer.env.environment(env).ok()?.get(point)
    }

    /// Snapshot every program-point fact of the analyzed callable.
    pub fn facts(&self) -> FactStore<ProgramPoint, V> {
        let transfer = self.driver.inner();
        transfer
            .root()
            .ok()
            .and_then(|(_, env)| transfer.env.environment(env).ok())
            .cloned()
            .unwrap_or_default()
    }
}

impl<'ir, S, V, E, F, Lk, Sem> DenseBackwardInterpreter<'ir, S, V, E, F, Lk, Sem>
where
    S: StageMeta + StageQuery + InterpDispatch<DenseBackwardTransfer<'ir, S, V, E, F, Sem>>,
    V: Clone + PartialEq + Lattice + HasBottom + PointFacts,
    E: From<InterpreterError>,
    Lk: Linker<S>,
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

    /// Resolve a stage and function by name, then seed and solve its blocks.
    pub fn analyze_by_name(
        self,
        stage_name: &str,
        function_name: &str,
    ) -> Result<(LinkTarget, Self), E> {
        let stage = self
            .driver
            .inner()
            .pipeline()
            .stage_by_name(stage_name)
            .ok_or_else(|| E::from(InterpreterError::MissingStageName(stage_name.into())))?;
        let function = self
            .driver
            .inner()
            .pipeline()
            .lookup_function_by_name(function_name)
            .ok_or_else(|| E::from(InterpreterError::MissingFunctionName(function_name.into())))?;
        self.analyze(stage, Callee::Function(function))
    }

    /// Seed and solve a callable body named by a stage-local symbol.
    pub fn analyze_by_symbol(
        self,
        stage: CompileStage,
        symbol: Symbol,
    ) -> Result<(LinkTarget, Self), E> {
        self.analyze(stage, symbol.into())
    }

    /// Resolve `callee`, seed every block selected by its CFG or Block body,
    /// and drain the block worklist. Dependencies are discovered from
    /// terminator edges; unsupported graph roots fail before solving.
    ///
    /// Each call solves from fresh state. It consumes the engine and returns
    /// it on success, like the other analysis engines; an error drops it.
    pub fn analyze(mut self, stage: CompileStage, callee: Callee) -> Result<(LinkTarget, Self), E> {
        let pipeline = self.driver.inner().pipeline();
        let target = self.linker.resolve(pipeline, stage, &callee)?;
        let body = target.body(pipeline)?;
        let blocks = self.direct_body_blocks(target.stage, body)?;

        let mut transfer = DenseBackwardTransfer::new(pipeline);
        let env = transfer.env.get_or_allocate(target);
        transfer.root = Some((target, env));
        self.driver = StandardFixpointInterpreter::new(transfer, DenseReaders::new());

        let work = blocks.into_iter().map(|block| (env, Body::Block(block)));
        self.driver
            .solve_many(&mut DenseBackwardWorkAnalysis, work)?;
        Ok((target, self))
    }
}
