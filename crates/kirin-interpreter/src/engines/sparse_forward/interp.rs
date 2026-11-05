//! The forward lattice-based abstract interpreter.
//!
//! # Layering
//!
//! [`SparseForwardInterpreter`] is the public engine. Internally it is a thin
//! wrapper over a [`StandardFixpointInterpreter`] driving a
//! [`SparseForwardTransfer`]:
//!
//! - **[`SparseForwardTransfer`]** is the [`Interp`] delegate and the target
//!   of rule dispatch: pipeline, linker, the [`EnvStore`] container (one
//!   environment per analysis context, addressed by the policy's context key,
//!   holding SSA facts), the contexts (entry, return, and callers), analysis
//!   policy, read/write logging, and the SSA facts that changed; it provides
//!   the dialect-dispatch / IR-query surface ([`StatementDispatch`], [`BlockQueries`], [`CFGQueries`],
//!   [`DiGraphQueries`], and — for concrete-shaped callers — [`CallServices`]).
//! - the **[`StandardFixpointInterpreter`]** driver owns the worklist and the
//!   readers of each SSA fact (blocks that read a value they do not define);
//!   after each walk it drains the changed facts and reruns their readers.
//!
//! # Work items
//!
//! A work item is `(EnvIndex, Body)`: a CFG block walk ([`Body::Block`]), or one
//! dependency-ordered pass over a graph body ([`Body::DiGraph`]). Writes join
//! into the environment ([`EnvStore::join`]); entries — a call's arguments, a
//! CFG edge's arguments — merge into the unit's parameters through the
//! analysis policy (join, then widen by visits) and schedule the unit on first
//! reach or when a parameter rose. A reached `ret` joins slot by slot into its
//! context's return and, if it rose, reruns the context's callers. Direct
//! dominated cross-block SSA uses are tracked by read logging, so a reader
//! block reruns when a value it read directly rises.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::hash::Hash;
use std::marker::PhantomData;

use kirin_ir::{
    Block, CFG, CompileStage, HasBottom, Pipeline, Product, SSAValue, SpecializedFunction,
    StageMeta, Statement, Symbol, Widen,
};

use crate::core::query;
use crate::{
    AbstractBlockFrame, AbstractCompletion, AbstractDiGraphFrame, AbstractInterpreter,
    BlockQueries, Body, CFGQueries, CallEffect, CallServices, Callee, DiGraphQueries, Env,
    EnvIndex, EnvStore, FixpointProfile, ForwardDataflowFrameEngine, ForwardEval, Frame, Interp,
    InterpDispatch, InterpLocation, InterpreterError, LinkTarget, Linker, SSABinding,
    SameStageLinker, SparseForwardEffect, SparseForwardSemantic, StageQuery, StandardAbstractFrame,
    StandardFixpointInterpreter, StatementDispatch, WorkAnalysis,
};

// ===========================================================================
// Pluggable analysis seams (policy `P`)
// ===========================================================================

/// Context-key strategy: maps a resolved call target plus its abstract arguments
/// to the key that selects that function context's environment.
pub trait CallContext<V> {
    type Key: Clone + Eq + Hash;

    fn key(&mut self, target: &LinkTarget, args: &Product<V>) -> Self::Key;
}

/// Explore/join strategy: combines an `incoming` abstract state into the
/// `current` state at a merge point, deciding join vs. widening from `visits`.
pub trait WideningStrategy<V> {
    fn merge(
        &self,
        current: &Product<V>,
        incoming: &Product<V>,
        visits: usize,
    ) -> Result<Product<V>, InterpreterError>;
}

/// Default analysis: context-insensitive keys and join-until-`widen_after` then
/// widen.
#[derive(Clone, Copy, Debug)]
pub struct ContextInsensitive {
    pub widen_after: usize,
}

impl Default for ContextInsensitive {
    fn default() -> Self {
        Self { widen_after: 3 }
    }
}

/// One context per resolved [`LinkTarget`], shared by every call site of that
/// target.
///
/// The target *is* the context-insensitive key: it already identifies the stage
/// and specialization a call resolved to, so re-deriving a tuple from it would
/// only be a second spelling of the same identity. Context sensitivity is what
/// *adds* to this key (see `ConstPropContext`), never what re-spells it.
impl<V> CallContext<V> for ContextInsensitive {
    type Key = LinkTarget;

    fn key(&mut self, target: &LinkTarget, _args: &Product<V>) -> Self::Key {
        *target
    }
}

impl<V> WideningStrategy<V> for ContextInsensitive
where
    V: Clone + Widen,
{
    fn merge(
        &self,
        current: &Product<V>,
        incoming: &Product<V>,
        visits: usize,
    ) -> Result<Product<V>, InterpreterError> {
        join_products(current, incoming, visits > self.widen_after)
    }
}

// ===========================================================================
// Anchors, contexts, work items
// ===========================================================================

/// One selected function context: what it analyzes, what it returns, and who
/// reads the return.
struct ForwardContext<V> {
    stage: CompileStage,
    /// The unit a call enters: the CFG's entry block, a block body, or a graph.
    entry: Body,
    /// The join of every reached return; `None` means no return reached yet,
    /// which differs from a reached zero-value return.
    ret: Option<Product<V>>,
    /// The walks that call this context, rerun when `ret` rises.
    callers: Vec<ForwardWork>,
}

/// One forward work item: walk `Body` (a CFG block as [`Body::Block`], or a
/// graph as [`Body::DiGraph`]) in the environment `EnvIndex`.
pub(super) type ForwardWork = (EnvIndex, Body);

/// The driver's `store` slot: the walks to rerun when an SSA fact rises —
/// blocks that read a value they do not define.
pub(super) type ForwardReaders = HashMap<(EnvIndex, SSAValue), Vec<ForwardWork>>;

/// The work-item type family for the forward engine. Facts live in the
/// transfer's [`EnvStore`].
pub struct SparseForwardProfile<V, E, F> {
    #[allow(clippy::type_complexity)]
    _marker: PhantomData<fn() -> (V, E, F)>,
}

impl<'ir, S, V, E, Lk, P, F, Sem>
    FixpointProfile<SparseForwardTransfer<'ir, S, V, E, Lk, P, F, Sem>>
    for SparseForwardProfile<V, E, F>
where
    S: StageMeta,
    V: Clone + HasBottom,
    E: From<InterpreterError>,
    P: CallContext<V>,
    Sem: SparseForwardSemantic,
{
    type Work = ForwardWork;
    type Frame = F;
    type Completion = AbstractCompletion<V>;
}

/// The forward driver: a [`StandardFixpointInterpreter`] over
/// [`SparseForwardTransfer`] whose work items are block and graph walks, with
/// the readers of each fact in its `store` slot.
type ForwardDriver<'ir, S, V, E, Lk, P, F, Sem> = StandardFixpointInterpreter<
    SparseForwardTransfer<'ir, S, V, E, Lk, P, F, Sem>,
    SparseForwardProfile<V, E, F>,
    ForwardReaders,
>;

// ===========================================================================
// SparseForwardTransfer — facts, metadata, and rule dispatch
// ===========================================================================

/// The transfer of the forward abstract interpreter: pipeline, linker, the
/// facts of every selected context, their metadata, the analysis policy, and
/// the per-walk logs. Dialect rules dispatch on it.
pub struct SparseForwardTransfer<
    'ir,
    S: StageMeta,
    V,
    E,
    Lk = SameStageLinker,
    P = ContextInsensitive,
    F = StandardAbstractFrame<V, E>,
    Sem = ForwardEval,
> where
    P: CallContext<V>,
    Sem: SparseForwardSemantic,
{
    pipeline: &'ir Pipeline<S>,
    linker: Lk,
    /// One environment per analysis context, addressed by the policy's context
    /// key. Blocks of the same context share it (so direct dominated
    /// cross-block uses resolve); distinct contexts are isolated.
    env: EnvStore<<P as CallContext<V>>::Key, SSAValue, V>,
    /// Selected contexts, by environment.
    contexts: HashMap<EnvIndex, ForwardContext<V>>,
    /// Entry merges per unit, for widening by visits. A unit is reached once
    /// it has an entry here.
    entry_joins: HashMap<ForwardWork, usize>,
    analysis: P,
    max_iterations: usize,
    location: Option<InterpLocation>,
    /// SSA values read during the current walk (interior-mutable because
    /// [`Env::env_read`] takes `&self`).
    read_log: RefCell<Vec<SSAValue>>,
    /// SSA values written during the current walk.
    write_log: Vec<SSAValue>,
    /// SSA facts that rose since the driver last woke their readers.
    changed: Vec<(EnvIndex, SSAValue)>,
    /// Whether read/write logging is active (only during a walk).
    logging: bool,
    #[allow(clippy::type_complexity)]
    _marker: PhantomData<fn() -> (E, F, Sem)>,
}

impl<'ir, S: StageMeta, V, E, P, F, Sem>
    SparseForwardTransfer<'ir, S, V, E, SameStageLinker, P, F, Sem>
where
    P: CallContext<V> + Default,
    Sem: SparseForwardSemantic,
{
    fn new(pipeline: &'ir Pipeline<S>) -> Self {
        Self::with_parts(pipeline, SameStageLinker, P::default())
    }
}

impl<'ir, S: StageMeta, V, E, Lk, P, F, Sem> SparseForwardTransfer<'ir, S, V, E, Lk, P, F, Sem>
where
    P: CallContext<V>,
    Sem: SparseForwardSemantic,
{
    fn with_parts(pipeline: &'ir Pipeline<S>, linker: Lk, analysis: P) -> Self {
        Self {
            pipeline,
            linker,
            env: EnvStore::new(),
            contexts: HashMap::new(),
            entry_joins: HashMap::new(),
            analysis,
            max_iterations: 1000,
            location: None,
            read_log: RefCell::new(Vec::new()),
            write_log: Vec::new(),
            changed: Vec::new(),
            logging: false,
            _marker: PhantomData,
        }
    }

    /// Swap the linker, resetting the analysis state.
    fn with_linker<Lk2>(self, linker: Lk2) -> SparseForwardTransfer<'ir, S, V, E, Lk2, P, F, Sem> {
        let mut transfer = SparseForwardTransfer::with_parts(self.pipeline, linker, self.analysis);
        transfer.max_iterations = self.max_iterations;
        transfer
    }

    /// Swap the analysis policy (and so the context key type), resetting the
    /// analysis state.
    #[allow(clippy::type_complexity)]
    fn with_analysis<P2>(
        self,
        analysis: P2,
    ) -> SparseForwardTransfer<'ir, S, V, E, Lk, P2, StandardAbstractFrame<V, E>, Sem>
    where
        P2: CallContext<V>,
    {
        let mut transfer = SparseForwardTransfer::with_parts(self.pipeline, self.linker, analysis);
        transfer.max_iterations = self.max_iterations;
        transfer
    }

    /// Replace the policy value, resetting the analysis state.
    fn with_policy(self, analysis: P) -> Self {
        let mut transfer = Self::with_parts(self.pipeline, self.linker, analysis);
        transfer.max_iterations = self.max_iterations;
        transfer
    }

    fn pipeline(&self) -> &'ir Pipeline<S> {
        self.pipeline
    }

    /// A selected context's metadata.
    fn context(&self, env: EnvIndex) -> Result<&ForwardContext<V>, InterpreterError> {
        self.contexts.get(&env).ok_or(UNSELECTED)
    }

    /// The fact of `value` in `env`, without logging; absent is bottom.
    fn fact<E2>(&self, env: EnvIndex, value: SSAValue) -> Result<V, E2>
    where
        V: Clone + HasBottom,
        E2: From<InterpreterError>,
    {
        Ok(self.env.read(env, value)?.unwrap_or_else(V::bottom))
    }

    /// Begin logging reads/writes for a walk.
    fn begin_block_log(&mut self) {
        self.logging = true;
        self.read_log.borrow_mut().clear();
        self.write_log.clear();
    }

    /// Stop logging and take the accumulated `(reads, writes)`.
    fn take_logs(&mut self) -> (Vec<SSAValue>, Vec<SSAValue>) {
        self.logging = false;
        let reads = std::mem::take(&mut *self.read_log.borrow_mut());
        let writes = std::mem::take(&mut self.write_log);
        (reads, writes)
    }
}

impl<'ir, S, V, E, Lk, P, F, Sem> SparseForwardTransfer<'ir, S, V, E, Lk, P, F, Sem>
where
    S: StageQuery,
    E: From<InterpreterError>,
    P: CallContext<V>,
    Sem: SparseForwardSemantic,
{
    /// The SSA values a unit's entry binds: a block's parameters, or a
    /// graph's boundary ports.
    fn entry_params(&self, stage: CompileStage, body: Body) -> Result<Vec<SSAValue>, E> {
        match body {
            Body::Block(block) => Ok(query::block_params(self.pipeline, stage, block)?),
            Body::DiGraph(graph) => Ok(query::digraph_walk_plan(self.pipeline, stage, graph)?
                .ports
                .into_iter()
                .map(SSAValue::from)
                .collect()),
            _ => Err(E::from(InterpreterError::NoDefaultWalker(body))),
        }
    }
}

// Policy-driven merge, kept on the transfer (the analysis `P`
// lives here). The driver's `ForwardDataflowFrameEngine` impl delegates to these.
impl<'ir, S: StageMeta, V, E, Lk, P, F, Sem> SparseForwardTransfer<'ir, S, V, E, Lk, P, F, Sem>
where
    V: Clone + PartialEq + Widen,
    E: From<InterpreterError>,
    P: CallContext<V> + WideningStrategy<V>,
    Sem: SparseForwardSemantic,
{
    fn merge_products(
        &self,
        current: &Product<V>,
        incoming: &Product<V>,
        visits: usize,
    ) -> Result<Product<V>, E> {
        self.analysis
            .merge(current, incoming, visits)
            .map_err(E::from)
    }

    /// Key a resolved call target through the analysis.
    fn key(&mut self, target: &LinkTarget, args: &Product<V>) -> <P as CallContext<V>>::Key {
        self.analysis.key(target, args)
    }
}

impl<'ir, S, V, E, Lk, P, F, Sem> Interp for SparseForwardTransfer<'ir, S, V, E, Lk, P, F, Sem>
where
    S: StageMeta,
    V: Clone + HasBottom,
    E: From<InterpreterError>,
    P: CallContext<V>,
    Sem: SparseForwardSemantic,
{
    type Value = V;
    type Error = E;
    type Effect = SparseForwardEffect<V, F>;
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

impl<'ir, S, V, E, Lk, P, F, Sem> Env for SparseForwardTransfer<'ir, S, V, E, Lk, P, F, Sem>
where
    S: StageMeta,
    V: Clone + HasBottom,
    E: From<InterpreterError>,
    P: CallContext<V>,
    Sem: SparseForwardSemantic,
{
    /// The sparse-forward shape anchors its facts on SSA values.
    type Anchor = SSAValue;

    fn env_read(&self, index: EnvIndex, value: SSAValue) -> Result<V, E> {
        // Log the read regardless of whether it resolves to a bound value or
        // bottom — an unbound read of a value defined elsewhere is exactly the
        // cross-block dependency to track.
        if self.logging {
            self.read_log.borrow_mut().push(value);
        }
        // An absent binding is bottom, not an error: this analysis may reach a
        // use before the definition's walk has run. An invalid environment
        // handle is still an error.
        self.fact(index, value)
    }

    /// An abstract write is a contribution: it joins into the stored fact.
    fn env_write(&mut self, index: EnvIndex, value: SSAValue, data: V) -> Result<(), E> {
        if self.env.join(index, value, &data)? {
            self.changed.push((index, value));
        }
        if self.logging {
            self.write_log.push(value);
        }
        Ok(())
    }
}

impl<'ir, S, V, E, Lk, P, F, Sem> AbstractInterpreter
    for SparseForwardTransfer<'ir, S, V, E, Lk, P, F, Sem>
where
    S: StageMeta,
    V: Clone + HasBottom,
    E: From<InterpreterError>,
    P: CallContext<V>,
    Sem: SparseForwardSemantic,
{
}

// The IR-query / dispatch capability surface. Dialect rules dispatch on the
// transfer. The transfer implements the *concrete* call lifecycle too, even
// though the abstract frames never use it: `SparseForwardTransfer` is also the
// engine a concrete-shaped caller can drive, and keeping it whole preserves the
// existing delegation to `ForwardDriver` unchanged.
impl<'ir, S, V, E, Lk, P, F, Sem> CallServices
    for SparseForwardTransfer<'ir, S, V, E, Lk, P, F, Sem>
where
    S: StageQuery + InterpDispatch<Self>,
    V: Clone + HasBottom,
    E: From<InterpreterError>,
    Lk: Linker<S>,
    P: CallContext<V>,
    Sem: SparseForwardSemantic,
{
    fn alloc_env(&mut self) -> EnvIndex {
        self.env.alloc()
    }

    fn free_env(&mut self, index: EnvIndex) -> Result<(), E> {
        self.env.free(index).map_err(E::from)
    }

    fn resolve_callee(&self, lookup_stage: CompileStage, callee: &Callee) -> Result<LinkTarget, E> {
        self.linker
            .resolve(self.pipeline, lookup_stage, callee)
            .map_err(E::from)
    }

    fn discover_body(&self, target: &LinkTarget) -> Result<Body, E> {
        target.body(self.pipeline).map_err(E::from)
    }
}

impl<'ir, S, V, E, Lk, P, F, Sem> StatementDispatch
    for SparseForwardTransfer<'ir, S, V, E, Lk, P, F, Sem>
where
    S: StageQuery + InterpDispatch<Self>,
    V: Clone + HasBottom,
    E: From<InterpreterError>,
    Lk: Linker<S>,
    P: CallContext<V>,
    Sem: SparseForwardSemantic,
{
    fn run_statement(
        &mut self,
        stage: CompileStage,
        statement: Statement,
        index: EnvIndex,
    ) -> Result<Self::Effect, E> {
        let pipeline = self.pipeline;
        let info = pipeline
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

impl<'ir, S, V, E, Lk, P, F, Sem> BlockQueries
    for SparseForwardTransfer<'ir, S, V, E, Lk, P, F, Sem>
where
    S: StageQuery + InterpDispatch<Self>,
    V: Clone + HasBottom,
    E: From<InterpreterError>,
    Lk: Linker<S>,
    P: CallContext<V>,
    Sem: SparseForwardSemantic,
{
    fn block_params(&self, stage: CompileStage, block: Block) -> Result<Vec<SSAValue>, E> {
        query::block_params(self.pipeline, stage, block).map_err(E::from)
    }

    fn first_statement(&self, stage: CompileStage, block: Block) -> Result<Option<Statement>, E> {
        query::first_statement(self.pipeline, stage, block).map_err(E::from)
    }

    fn next_statement(
        &self,
        stage: CompileStage,
        block: Block,
        after: Statement,
    ) -> Result<Option<Statement>, E> {
        query::next_statement(self.pipeline, stage, block, after).map_err(E::from)
    }
}

impl<'ir, S, V, E, Lk, P, F, Sem> CFGQueries for SparseForwardTransfer<'ir, S, V, E, Lk, P, F, Sem>
where
    S: StageQuery + InterpDispatch<Self>,
    V: Clone + HasBottom,
    E: From<InterpreterError>,
    Lk: Linker<S>,
    P: CallContext<V>,
    Sem: SparseForwardSemantic,
{
    fn cfg_entry(&self, stage: CompileStage, cfg: CFG) -> Result<Option<Block>, E> {
        query::cfg_entry(self.pipeline, stage, cfg).map_err(E::from)
    }
}

impl<'ir, S, V, E, Lk, P, F, Sem> DiGraphQueries
    for SparseForwardTransfer<'ir, S, V, E, Lk, P, F, Sem>
where
    S: StageQuery + InterpDispatch<Self>,
    V: Clone + HasBottom,
    E: From<InterpreterError>,
    Lk: Linker<S>,
    P: CallContext<V>,
    Sem: SparseForwardSemantic,
{
    fn digraph_walk_plan(
        &self,
        stage: CompileStage,
        graph: kirin_ir::DiGraph,
    ) -> Result<crate::GraphWalkPlan, E> {
        query::digraph_walk_plan(self.pipeline, stage, graph).map_err(E::from)
    }
}

// ===========================================================================
// Driver capability impls (frames run on the driver, which delegates to the transfer)
// ===========================================================================

// Delegation is unchanged; only the trait each group of methods belongs to.
impl<'ir, S, V, E, Lk, P, F, Sem> CallServices for ForwardDriver<'ir, S, V, E, Lk, P, F, Sem>
where
    S: StageQuery + InterpDispatch<SparseForwardTransfer<'ir, S, V, E, Lk, P, F, Sem>>,
    V: Clone + HasBottom,
    E: From<InterpreterError>,
    Lk: Linker<S>,
    P: CallContext<V>,
    Sem: SparseForwardSemantic,
{
    fn alloc_env(&mut self) -> EnvIndex {
        self.inner_mut().alloc_env()
    }

    fn free_env(&mut self, index: EnvIndex) -> Result<(), E> {
        self.inner_mut().free_env(index)
    }

    fn resolve_callee(&self, lookup_stage: CompileStage, callee: &Callee) -> Result<LinkTarget, E> {
        self.inner().resolve_callee(lookup_stage, callee)
    }

    fn discover_body(&self, target: &LinkTarget) -> Result<Body, E> {
        self.inner().discover_body(target)
    }
}

impl<'ir, S, V, E, Lk, P, F, Sem> StatementDispatch for ForwardDriver<'ir, S, V, E, Lk, P, F, Sem>
where
    S: StageQuery + InterpDispatch<SparseForwardTransfer<'ir, S, V, E, Lk, P, F, Sem>>,
    V: Clone + HasBottom,
    E: From<InterpreterError>,
    Lk: Linker<S>,
    P: CallContext<V>,
    Sem: SparseForwardSemantic,
{
    fn run_statement(
        &mut self,
        stage: CompileStage,
        statement: Statement,
        index: EnvIndex,
    ) -> Result<Self::Effect, E> {
        self.inner_mut().run_statement(stage, statement, index)
    }
}

impl<'ir, S, V, E, Lk, P, F, Sem> BlockQueries for ForwardDriver<'ir, S, V, E, Lk, P, F, Sem>
where
    S: StageQuery + InterpDispatch<SparseForwardTransfer<'ir, S, V, E, Lk, P, F, Sem>>,
    V: Clone + HasBottom,
    E: From<InterpreterError>,
    Lk: Linker<S>,
    P: CallContext<V>,
    Sem: SparseForwardSemantic,
{
    fn block_params(&self, stage: CompileStage, block: Block) -> Result<Vec<SSAValue>, E> {
        self.inner().block_params(stage, block)
    }

    fn first_statement(&self, stage: CompileStage, block: Block) -> Result<Option<Statement>, E> {
        self.inner().first_statement(stage, block)
    }

    fn next_statement(
        &self,
        stage: CompileStage,
        block: Block,
        after: Statement,
    ) -> Result<Option<Statement>, E> {
        self.inner().next_statement(stage, block, after)
    }
}

impl<'ir, S, V, E, Lk, P, F, Sem> CFGQueries for ForwardDriver<'ir, S, V, E, Lk, P, F, Sem>
where
    S: StageQuery + InterpDispatch<SparseForwardTransfer<'ir, S, V, E, Lk, P, F, Sem>>,
    V: Clone + HasBottom,
    E: From<InterpreterError>,
    Lk: Linker<S>,
    P: CallContext<V>,
    Sem: SparseForwardSemantic,
{
    fn cfg_entry(&self, stage: CompileStage, cfg: CFG) -> Result<Option<Block>, E> {
        self.inner().cfg_entry(stage, cfg)
    }
}

impl<'ir, S, V, E, Lk, P, F, Sem> DiGraphQueries for ForwardDriver<'ir, S, V, E, Lk, P, F, Sem>
where
    S: StageQuery + InterpDispatch<SparseForwardTransfer<'ir, S, V, E, Lk, P, F, Sem>>,
    V: Clone + HasBottom,
    E: From<InterpreterError>,
    Lk: Linker<S>,
    P: CallContext<V>,
    Sem: SparseForwardSemantic,
{
    fn digraph_walk_plan(
        &self,
        stage: CompileStage,
        graph: kirin_ir::DiGraph,
    ) -> Result<crate::GraphWalkPlan, E> {
        self.inner().digraph_walk_plan(stage, graph)
    }
}

impl<'ir, S, V, E, Lk, P, F, Sem> ForwardDataflowFrameEngine
    for ForwardDriver<'ir, S, V, E, Lk, P, F, Sem>
where
    S: StageQuery + InterpDispatch<SparseForwardTransfer<'ir, S, V, E, Lk, P, F, Sem>>,
    V: Clone + PartialEq + Widen + HasBottom,
    E: From<InterpreterError>,
    Lk: Linker<S>,
    P: CallContext<V> + WideningStrategy<V>,
    Sem: SparseForwardSemantic,
{
    fn analysis_merge(
        &self,
        current: &Product<V>,
        incoming: &Product<V>,
        visits: usize,
    ) -> Result<Product<V>, E> {
        self.inner().merge_products(current, incoming, visits)
    }

    fn contribute_return(&mut self, values: Product<V>) -> Result<(), E> {
        let (env, _) = *self.current_work().ok_or_else(|| {
            E::from(InterpreterError::Custom(
                "a return was reached outside propagation work",
            ))
        })?;
        self.raise_return(env, values)
    }

    /// Summarize a call atomically: resolve, merge the callee entry (which
    /// schedules its entry unit), register the caller on the callee's return
    /// (including same-key self-recursion), and bind the callee's current
    /// return (or bottom per slot).
    fn summarize_call(
        &mut self,
        stage: CompileStage,
        call: CallEffect<V>,
        index: EnvIndex,
    ) -> Result<(), E> {
        let CallEffect {
            callee,
            stage: call_stage,
            inputs: args,
            outputs: results,
        } = call;
        let lookup_stage = call_stage.unwrap_or(stage);
        let callee_env = self.contribute_call_entry(lookup_stage, &callee, args)?;

        let caller = *self.current_work().ok_or_else(|| {
            E::from(InterpreterError::Custom(
                "a call boundary was crossed outside propagation work",
            ))
        })?;
        let context = self
            .inner_mut()
            .contexts
            .get_mut(&callee_env)
            .ok_or_else(|| E::from(UNSELECTED))?;
        if !context.callers.contains(&caller) {
            context.callers.push(caller);
        }

        match context.ret.clone() {
            Some(values) => self.bind_values(index, results.as_slice(), values),
            None => {
                for slot in results.iter().copied() {
                    self.env_write(index, slot, V::bottom())?;
                }
                Ok(())
            }
        }
    }

    fn max_iterations(&self) -> usize {
        self.inner().max_iterations
    }
}

// Every fact change and every reschedule flows through these driver methods:
// SSA facts join on write (`Env::env_write`), entries merge through the
// analysis policy (join or widen by visits), and returns join per slot.
impl<'ir, S, V, E, Lk, P, F, Sem> ForwardDriver<'ir, S, V, E, Lk, P, F, Sem>
where
    S: StageQuery + InterpDispatch<SparseForwardTransfer<'ir, S, V, E, Lk, P, F, Sem>>,
    V: Clone + PartialEq + Widen + HasBottom,
    E: From<InterpreterError>,
    Lk: Linker<S>,
    P: CallContext<V> + WideningStrategy<V>,
    Sem: SparseForwardSemantic,
{
    /// Resolve and key a call, select (allocating on first use) the context's
    /// environment, and merge the arguments into its entry unit.
    fn contribute_call_entry(
        &mut self,
        lookup_stage: CompileStage,
        callee: &Callee,
        args: Product<V>,
    ) -> Result<EnvIndex, E> {
        let target = self.inner().resolve_callee(lookup_stage, callee)?;
        let body = target.body(self.inner().pipeline()).map_err(E::from)?;
        let key = self.inner_mut().key(&target, &args);
        let env = self.inner_mut().env.get_or_allocate(key);
        if !self.inner().contexts.contains_key(&env) {
            // The one place a function becomes runnable work: a CFG's entry
            // block or a block body walk as a block; a graph is its own unit;
            // an undirected graph has no traversal order.
            let entry = match body {
                Body::CFG(cfg) => Body::Block(
                    self.cfg_entry(target.stage, cfg)?
                        .ok_or_else(|| E::from(InterpreterError::EmptyCFG))?,
                ),
                Body::Block(_) | Body::DiGraph(_) => body,
                Body::UnGraph(_) => {
                    return Err(E::from(InterpreterError::NoDefaultWalker(body)));
                }
            };
            self.inner_mut().contexts.insert(
                env,
                ForwardContext {
                    stage: target.stage,
                    entry,
                    ret: None,
                    callers: Vec::new(),
                },
            );
        }
        let entry = self.inner().context(env)?.entry;
        self.merge_entry((env, entry), args)?;
        Ok(env)
    }

    /// Merge incoming arguments into a unit's entry parameters through the
    /// analysis policy (join, then widen by visits). Schedule the unit on its
    /// first reach or when a parameter rose, and record every parameter that
    /// changed.
    fn merge_entry(&mut self, work: ForwardWork, args: Product<V>) -> Result<(), E> {
        let (env, body) = work;
        let stage = self.inner().context(env)?.stage;
        let params = self.inner().entry_params(stage, body)?;
        if params.len() != args.len() {
            return Err(E::from(match body {
                Body::Block(block) => InterpreterError::BlockArityMismatch {
                    block,
                    expected: params.len(),
                    actual: args.len(),
                },
                _ => InterpreterError::ProductArityMismatch {
                    expected: params.len(),
                    actual: args.len(),
                },
            }));
        }
        let current: Product<V> = params
            .iter()
            .map(|param| self.inner().fact(env, *param))
            .collect::<Result<_, E>>()?;
        let visits = self.inner().entry_joins.get(&work).copied();
        let merged = match visits {
            None => args,
            Some(visits) => self.inner().merge_products(&current, &args, visits + 1)?,
        };
        self.inner_mut()
            .entry_joins
            .insert(work, visits.map_or(0, |visits| visits + 1));

        let mut rose = false;
        for ((param, old), new) in params.iter().zip(current.iter()).zip(merged) {
            if *old != new {
                rose = true;
                self.inner_mut()
                    .env
                    .write(env, *param, new)
                    .map_err(E::from)?;
                self.inner_mut().changed.push((env, *param));
            }
        }
        if visits.is_none() || rose {
            self.schedule_work(work);
        }
        Ok(())
    }

    /// Join a reached return into the function's return, slot by slot, and
    /// rerun its callers if it rose. Every return of one function must have
    /// the same arity.
    fn raise_return(&mut self, env: EnvIndex, values: Product<V>) -> Result<(), E> {
        let context = self
            .inner_mut()
            .contexts
            .get_mut(&env)
            .ok_or_else(|| E::from(UNSELECTED))?;
        let ret = context
            .ret
            .get_or_insert_with(|| values.iter().map(|_| V::bottom()).collect());
        if ret.len() != values.len() {
            return Err(E::from(InterpreterError::ProductArityMismatch {
                expected: ret.len(),
                actual: values.len(),
            }));
        }
        let rose = ret
            .iter_mut()
            .zip(values.iter())
            .fold(false, |rose, (slot, value)| slot.join_assign(value) | rose);
        if rose {
            for caller in context.callers.clone() {
                self.schedule_work(caller);
            }
        }
        Ok(())
    }

    /// Register `work` as a reader of `value` in `env`.
    fn subscribe(&mut self, env: EnvIndex, value: SSAValue, work: ForwardWork) {
        let readers = self.store_mut().entry((env, value)).or_default();
        if !readers.contains(&work) {
            readers.push(work);
        }
    }

    /// Schedule the readers of every SSA fact that changed.
    fn wake_changed(&mut self) {
        for key in std::mem::take(&mut self.inner_mut().changed) {
            let readers = self.store().get(&key).cloned().unwrap_or_default();
            for reader in readers {
                self.schedule_work(reader);
            }
        }
    }

    fn schedule_work(&mut self, work: ForwardWork) {
        self.schedule(work);
    }
}

const UNSELECTED: InterpreterError =
    InterpreterError::Custom("forward work names an unselected context");

// ===========================================================================
// Work analysis: block and graph walks are the work items.
// ===========================================================================

/// The forward work analysis: bind the unit's current entry parameters, walk
/// it once, then register its cross-block reads, merge its edges into
/// successors, and wake the readers of what changed.
struct SparseForwardWorkAnalysis<V> {
    _marker: PhantomData<fn() -> V>,
}

impl<V> SparseForwardWorkAnalysis<V> {
    fn new() -> Self {
        Self {
            _marker: PhantomData,
        }
    }
}

impl<'ir, S, V, E, Lk, P, F, Sem>
    WorkAnalysis<
        ForwardDriver<'ir, S, V, E, Lk, P, F, Sem>,
        ForwardWork,
        F,
        AbstractCompletion<V>,
        E,
    > for SparseForwardWorkAnalysis<V>
where
    S: StageQuery + InterpDispatch<SparseForwardTransfer<'ir, S, V, E, Lk, P, F, Sem>>,
    V: Clone + PartialEq + Widen + HasBottom,
    E: From<InterpreterError>,
    Lk: Linker<S>,
    P: CallContext<V> + WideningStrategy<V>,
    Sem: SparseForwardSemantic,
    F: From<AbstractBlockFrame<V, E>> + From<AbstractDiGraphFrame<V, E>>,
{
    fn entry_frame(
        &mut self,
        interp: &mut ForwardDriver<'ir, S, V, E, Lk, P, F, Sem>,
        &(env, body): &ForwardWork,
    ) -> Result<F, E> {
        let stage = interp.inner().context(env)?.stage;
        let args: Product<V> = interp
            .inner()
            .entry_params(stage, body)?
            .into_iter()
            .map(|param| interp.inner().fact(env, param))
            .collect::<Result<_, E>>()?;
        interp.inner_mut().begin_block_log();
        match body {
            Body::Block(block) => {
                Ok(AbstractBlockFrame::new_cfg_block(stage, env, block, args).into())
            }
            // One dependency-ordered pass over the whole graph. Exact for a DAG,
            // so the pass never needs to iterate internally.
            Body::DiGraph(graph) => Ok(AbstractDiGraphFrame::new(stage, env, graph, args).into()),
            _ => Err(E::from(InterpreterError::Custom(
                "forward work must be a block or a directed graph",
            ))),
        }
    }

    fn complete_work(
        &mut self,
        interp: &mut ForwardDriver<'ir, S, V, E, Lk, P, F, Sem>,
        work: ForwardWork,
        completion: AbstractCompletion<V>,
    ) -> Result<(), E> {
        let env = work.0;
        // A block completes with its outgoing CFG edges. A graph has no
        // successors at all — it completes with its declared yields, which for
        // a callable graph body *are* the function's return values.
        let (edges, graph_yields) = match (work.1, completion) {
            (Body::Block(_), AbstractCompletion::CFGBlock { edges }) => (edges, None),
            (Body::DiGraph(_), AbstractCompletion::Finished(values)) => (Vec::new(), values),
            _ => {
                return Err(E::from(InterpreterError::Custom(
                    "forward work completed with a mismatched completion",
                )));
            }
        };
        if let Some(values) = graph_yields {
            interp.raise_return(env, values)?;
        }

        // Register the walk's reads of values it did not write before waking
        // the readers of what changed, so a walk that reads a fact it then
        // raises reruns.
        let (reads, writes) = interp.inner_mut().take_logs();
        let written: HashSet<SSAValue> = writes.into_iter().collect();
        for value in reads {
            if !written.contains(&value) {
                interp.subscribe(env, value, work);
            }
        }

        // CFG edges merge into the successors' entry parameters. Empty for a
        // returning block and for a graph.
        for edge in edges {
            interp.merge_entry((env, Body::Block(edge.target)), edge.args)?;
        }
        interp.wake_changed();
        Ok(())
    }
}

// ===========================================================================
// The public engine: a wrapper over the driver
// ===========================================================================

/// Forward lattice-based abstract interpreter.
///
/// Drives the same forward dialect rules (`Interpretable<I, ForwardEval>`) and
/// [`SparseForwardEffect`] as concrete execution, over an abstract value domain.
/// Traversal is owned by the total frame type `F`; context keying and merge/widen
/// behavior are owned by the policy `P`.
///
/// `analyze` consumes the engine and hands it back only on success, so a
/// failed analysis can never be resumed: build a new engine instead.
/// Successful analyses accumulate facts, so a request whose policy key
/// matches an earlier context returns the join with the earlier requests
/// (for every repeated callee under [`ContextInsensitive`]) and can depend on
/// query order. Use a fresh engine for an isolated answer.
///
/// ```ignore
/// let analysis = SparseForwardInterpreter::<Stage, ConstPropValue<MyConst>, MyError>::new(&pipeline)
///     .with_linker(CrossStageLinker);
/// let (result, analysis) = analysis.analyze_by_name("source", "abs", [ConstPropValue::Const(value)])?;
/// ```
pub struct SparseForwardInterpreter<
    'ir,
    S: StageMeta,
    V,
    E,
    Lk = SameStageLinker,
    P = ContextInsensitive,
    F = StandardAbstractFrame<V, E>,
    Sem = ForwardEval,
> where
    V: Clone + HasBottom,
    E: From<InterpreterError>,
    P: CallContext<V>,
    Sem: SparseForwardSemantic,
{
    driver: ForwardDriver<'ir, S, V, E, Lk, P, F, Sem>,
}

impl<'ir, S: StageMeta, V, E, P, F, Sem>
    SparseForwardInterpreter<'ir, S, V, E, SameStageLinker, P, F, Sem>
where
    V: Clone + HasBottom,
    E: From<InterpreterError>,
    P: CallContext<V> + Default,
    Sem: SparseForwardSemantic,
{
    pub fn new(pipeline: &'ir Pipeline<S>) -> Self {
        Self {
            driver: StandardFixpointInterpreter::new(
                SparseForwardTransfer::new(pipeline),
                ForwardReaders::new(),
            ),
        }
    }
}

impl<'ir, S: StageMeta, V, E, Lk, P, F, Sem> SparseForwardInterpreter<'ir, S, V, E, Lk, P, F, Sem>
where
    V: Clone + HasBottom,
    E: From<InterpreterError>,
    P: CallContext<V>,
    Sem: SparseForwardSemantic,
{
    /// Swap the calling-convention component (the [`Linker`]). Preserves the frame
    /// type `F`; resets the analysis state.
    pub fn with_linker<Lk2>(
        self,
        linker: Lk2,
    ) -> SparseForwardInterpreter<'ir, S, V, E, Lk2, P, F, Sem> {
        let transfer = self.driver.into_inner().with_linker(linker);
        SparseForwardInterpreter {
            driver: StandardFixpointInterpreter::new(transfer, ForwardReaders::new()),
        }
    }

    /// Swap the analysis policy (context abstraction + join/widen). Changes the
    /// [`CallContext::Key`] type, so this resets the analysis state; the frame
    /// type resets to [`StandardAbstractFrame`].
    #[allow(clippy::type_complexity)]
    pub fn with_analysis<P2>(
        self,
        analysis: P2,
    ) -> SparseForwardInterpreter<'ir, S, V, E, Lk, P2, StandardAbstractFrame<V, E>, Sem>
    where
        P2: CallContext<V>,
    {
        let transfer = self.driver.into_inner().with_analysis(analysis);
        SparseForwardInterpreter {
            driver: StandardFixpointInterpreter::new(transfer, ForwardReaders::new()),
        }
    }

    /// Replace the analysis policy *value* while keeping its type (and so the
    /// [`CallContext::Key`]) and the frame type `F`. Resets the analysis state.
    pub fn with_policy(self, analysis: P) -> Self {
        let transfer = self.driver.into_inner().with_policy(analysis);
        Self {
            driver: StandardFixpointInterpreter::new(transfer, ForwardReaders::new()),
        }
    }

    pub fn pipeline(&self) -> &'ir Pipeline<S> {
        self.driver.inner().pipeline()
    }
}

impl<'ir, S: StageMeta, V, E, Lk, F, Sem>
    SparseForwardInterpreter<'ir, S, V, E, Lk, ContextInsensitive, F, Sem>
where
    V: Clone + HasBottom,
    E: From<InterpreterError>,
    Sem: SparseForwardSemantic,
{
    /// Number of joins at a merge point before switching from join to widening
    /// (only available with [`ContextInsensitive`]).
    pub fn widen_after(mut self, joins: usize) -> Self {
        self.driver.inner_mut().analysis.widen_after = joins;
        self
    }

    /// The joined return of an analyzed function (context-insensitive keying
    /// only); `None` if it was never analyzed or never returned.
    pub fn return_summary(
        &self,
        stage: CompileStage,
        function: SpecializedFunction,
    ) -> Option<Product<V>> {
        let target = LinkTarget {
            stage,
            specialization: function,
        };
        let transfer = self.driver.inner();
        let env = transfer.env.context_env(&target)?;
        transfer.context(env).ok()?.ret.clone()
    }
}

impl<'ir, S, V, E, Lk, P, F, Sem> SparseForwardInterpreter<'ir, S, V, E, Lk, P, F, Sem>
where
    S: StageQuery + InterpDispatch<SparseForwardTransfer<'ir, S, V, E, Lk, P, F, Sem>>,
    V: Clone + PartialEq + Widen + HasBottom,
    E: From<InterpreterError>,
    Lk: Linker<S>,
    P: CallContext<V> + WideningStrategy<V>,
    Sem: SparseForwardSemantic,
    F: Frame<ForwardDriver<'ir, S, V, E, Lk, P, F, Sem>, F, Completion = AbstractCompletion<V>>
        + From<AbstractBlockFrame<V, E>>
        + From<AbstractDiGraphFrame<V, E>>,
{
    /// Resolve `stage`/`function` by name and analyze. Returns the function's
    /// inferred return product at the fixpoint (empty if it never returns).
    pub fn analyze_by_name(
        self,
        stage_name: &str,
        function_name: &str,
        args: impl IntoIterator<Item = V>,
    ) -> Result<(Product<V>, Self), E> {
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
        self.analyze(stage, Callee::Function(function), args)
    }

    /// Resolve a stage-local `symbol` through the linker and analyze the
    /// selected callable. This is the convenience form of
    /// [`analyze`](Self::analyze) with [`Callee::Named`], and returns its
    /// inferred return product at the fixpoint.
    pub fn analyze_by_symbol(
        self,
        stage: CompileStage,
        symbol: Symbol,
        args: impl IntoIterator<Item = V>,
    ) -> Result<(Product<V>, Self), E> {
        self.analyze(stage, symbol.into(), args)
    }

    /// Run the fixpoint from a single entry. Seeds the entry function's entry unit
    /// and drains the worklist. Returns the engine on success; an error drops
    /// it.
    pub fn analyze(
        mut self,
        stage: CompileStage,
        callee: Callee,
        args: impl IntoIterator<Item = V>,
    ) -> Result<(Product<V>, Self), E> {
        let args: Product<V> = args.into_iter().collect();
        let env = self.driver.contribute_call_entry(stage, &callee, args)?;
        self.driver.wake_changed();
        let mut analysis = SparseForwardWorkAnalysis::new();
        self.driver.drain_worklist(&mut analysis)?;

        let ret = self
            .driver
            .inner()
            .context(env)?
            .ret
            .clone()
            .unwrap_or_default();
        Ok((ret, self))
    }
}

/// Element-wise join (or widen) of two products of equal arity.
fn join_products<V>(
    old: &Product<V>,
    new: &Product<V>,
    widen: bool,
) -> Result<Product<V>, InterpreterError>
where
    V: Clone + Widen,
{
    if old.len() != new.len() {
        return Err(InterpreterError::ProductArityMismatch {
            expected: old.len(),
            actual: new.len(),
        });
    }
    Ok(old
        .iter()
        .zip(new.iter())
        .map(|(old, new)| if widen { old.widen(new) } else { old.join(new) })
        .collect())
}
