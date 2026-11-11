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
//!   policy, and the SSA facts that changed; it provides the dialect-dispatch
//!   / IR-query surface ([`StatementDispatch`], [`BlockQueries`],
//!   [`CFGQueries`], [`DiGraphQueries`], and — for concrete-shaped callers —
//!   [`CallServices`]).
//! - the **[`StandardFixpointInterpreter`]** driver owns the worklist; after
//!   each walk it drains the changed facts and reruns the reached units that
//!   use them.
//!
//! # Work items
//!
//! A work item is `(EnvIndex, Body)`: a CFG block walk ([`Body::Block`]), or one
//! dependency-ordered pass over a graph body ([`Body::DiGraph`]). Writes join
//! into the environment ([`EnvStore::join`]); entries — a call's arguments, a
//! CFG edge's arguments — merge into the unit's parameters through the
//! analysis policy (join, then widen by visits) and schedule the unit on first
//! reach or when a parameter rose. A reached `ret` joins slot by slot into its
//! context's return and, if it rose, reruns the context's callers. A rising SSA
//! fact reruns, through the IR's use list, the innermost reached unit around
//! each use outside the defining unit (uses in nested structured bodies map to
//! their enclosing unit), so direct dominated cross-block uses see it.

use std::collections::{HashMap, HashSet};
use std::hash::Hash;
use std::marker::PhantomData;

use kirin_ir::{
    Block, CFG, CompileStage, HasBottom, Pipeline, Product, SSAValue, StageMeta, Statement, Symbol,
    Widen,
};

use crate::core::query;
use crate::{
    AbstractBlockFrame, AbstractCompletion, AbstractDiGraphFrame, AbstractInterpreter,
    AnalysisResults, Analyzed, BlockQueries, Body, CFGQueries, CallEffect, CallServices, Callee,
    DiGraphQueries, Env, EnvIndex, EnvStore, FixpointProfile, ForwardDataflowFrameEngine,
    ForwardEval, Frame, Interp, InterpDispatch, InterpLocation, InterpreterError, LinkTarget,
    Linker, SSABinding, SameStageLinker, SparseForwardEffect, SparseForwardSemantic, StageQuery,
    StandardAbstractFrame, StandardFixpointInterpreter, StatementDispatch, WorkAnalysis,
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
    /// The callable this context analyzes.
    target: LinkTarget,
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
/// [`SparseForwardTransfer`] whose work items are block and graph walks. It
/// keeps no readers: the IR's use lists name them.
type ForwardDriver<'ir, S, V, E, Lk, P, F, Sem> = StandardFixpointInterpreter<
    SparseForwardTransfer<'ir, S, V, E, Lk, P, F, Sem>,
    SparseForwardProfile<V, E, F>,
    (),
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
    /// SSA facts that rose since the driver last reran their uses.
    changed: Vec<(EnvIndex, SSAValue)>,
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
            changed: Vec::new(),
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

    /// The fact of `value` in `env`; absent is bottom.
    fn fact<E2>(&self, env: EnvIndex, value: SSAValue) -> Result<V, E2>
    where
        V: Clone + HasBottom,
        E2: From<InterpreterError>,
    {
        Ok(self.env.read(env, value)?.unwrap_or_else(V::bottom))
    }

    /// The innermost unit of `units` reached in `env`.
    fn reached(&self, env: EnvIndex, units: &[Body]) -> Option<ForwardWork> {
        units
            .iter()
            .map(|body| (env, *body))
            .find(|work| self.entry_joins.contains_key(work))
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
                    target,
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
        let stage = self.inner().context(env)?.target.stage;
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

    /// Rerun every reached unit that uses an SSA fact that changed, except
    /// the unit defining it: a use there follows the definition in the same
    /// walk, and a rising entry parameter already scheduled its unit.
    fn wake_changed(&mut self) -> Result<(), E> {
        let mut woken = HashSet::new();
        for (env, value) in std::mem::take(&mut self.inner_mut().changed) {
            let stage = self.inner().context(env)?.target.stage;
            let (def, uses) = query::value_units(self.inner().pipeline(), stage, value)?;
            let def = self.inner().reached(env, &def);
            for units in uses {
                if let Some(unit) = self.inner().reached(env, &units)
                    && Some(unit) != def
                    && woken.insert(unit)
                {
                    self.schedule_work(unit);
                }
            }
        }
        Ok(())
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
/// successors, and rerun the uses of what changed.
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
        let stage = interp.inner().context(env)?.target.stage;
        let args: Product<V> = interp
            .inner()
            .entry_params(stage, body)?
            .into_iter()
            .map(|param| interp.inner().fact(env, param))
            .collect::<Result<_, E>>()?;
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

        // CFG edges merge into the successors' entry parameters. Empty for a
        // returning block and for a graph.
        for edge in edges {
            interp.merge_entry((env, Body::Block(edge.target)), edge.args)?;
        }
        interp.wake_changed()
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
/// `analyze` consumes the engine and returns the converged [`Analyzed`]
/// state, read through [`AnalysisResults`]. Each analysis is fresh: analyzing
/// another root takes another engine.
///
/// ```ignore
/// let analyzed = SparseForwardInterpreter::<Stage, ConstPropValue<MyConst>, MyError>::new(&pipeline)
///     .with_linker(CrossStageLinker)
///     .analyze_by_name("source", "abs", [ConstPropValue::Const(value)])?;
/// let result = analyzed.response(analyzed.root());
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
    /// The analyzed root's context key, set by `analyze`.
    root: Option<P::Key>,
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
            driver: StandardFixpointInterpreter::new(SparseForwardTransfer::new(pipeline), ()),
            root: None,
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
            driver: StandardFixpointInterpreter::new(transfer, ()),
            root: None,
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
            driver: StandardFixpointInterpreter::new(transfer, ()),
            root: None,
        }
    }

    /// Replace the analysis policy *value* while keeping its type (and so the
    /// [`CallContext::Key`]) and the frame type `F`. Resets the analysis state.
    pub fn with_policy(self, analysis: P) -> Self {
        let transfer = self.driver.into_inner().with_policy(analysis);
        Self {
            driver: StandardFixpointInterpreter::new(transfer, ()),
            root: None,
        }
    }

    pub fn pipeline(&self) -> &'ir Pipeline<S> {
        self.driver.inner().pipeline()
    }

    /// A selected context's environment and metadata.
    fn selected(&self, key: &P::Key) -> Option<(EnvIndex, &ForwardContext<V>)> {
        let transfer = self.driver.inner();
        let env = transfer.env.context_env(key)?;
        Some((env, transfer.contexts.get(&env)?))
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
    /// Resolve `stage`/`function` by name and analyze. See
    /// [`analyze`](Self::analyze).
    pub fn analyze_by_name(
        self,
        stage_name: &str,
        function_name: &str,
        args: impl IntoIterator<Item = V>,
    ) -> Result<Analyzed<Self>, E> {
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
    /// [`analyze`](Self::analyze) with [`Callee::Named`].
    pub fn analyze_by_symbol(
        self,
        stage: CompileStage,
        symbol: Symbol,
        args: impl IntoIterator<Item = V>,
    ) -> Result<Analyzed<Self>, E> {
        self.analyze(stage, symbol.into(), args)
    }

    /// Run the fixpoint from a single entry. Seeds the entry function's entry unit
    /// and drains the worklist. The inferred return product is the root's
    /// [`response`](AnalysisResults::response) (`None` if it never returns);
    /// an error drops the engine.
    pub fn analyze(
        mut self,
        stage: CompileStage,
        callee: Callee,
        args: impl IntoIterator<Item = V>,
    ) -> Result<Analyzed<Self>, E> {
        let args: Product<V> = args.into_iter().collect();
        let env = self.driver.contribute_call_entry(stage, &callee, args)?;
        self.driver.wake_changed()?;
        let mut analysis = SparseForwardWorkAnalysis::new();
        self.driver.drain_worklist(&mut analysis)?;

        let transfer = self.driver.inner();
        let root = transfer
            .env
            .key(env)
            .cloned()
            .ok_or_else(|| E::from(UNSELECTED))?;
        let contexts: Vec<_> = transfer
            .contexts
            .iter()
            .map(|(env, context)| (context.target, *env))
            .collect();
        self.root = Some(root);
        Ok(Analyzed::new(self, contexts))
    }
}

impl<'ir, S, V, E, Lk, P, F, Sem> AnalysisResults
    for Analyzed<SparseForwardInterpreter<'ir, S, V, E, Lk, P, F, Sem>>
where
    S: StageMeta,
    V: Clone + Widen,
    E: From<InterpreterError>,
    P: CallContext<V>,
    Sem: SparseForwardSemantic,
{
    type Context = P::Key;
    type Anchor = SSAValue;
    type Value = V;

    fn root(&self) -> &P::Key {
        self.engine
            .root
            .as_ref()
            .expect("`analyze` sets the root before building `Analyzed`")
    }

    /// The selected function contexts.
    fn contexts(&self) -> impl Iterator<Item = &P::Key> {
        let transfer = self.engine.driver.inner();
        transfer
            .env
            .contexts()
            .filter(|(_, env)| transfer.contexts.contains_key(env))
            .map(|(key, _)| key)
    }

    fn callable_of(&self, context: &P::Key) -> Option<LinkTarget> {
        Some(self.engine.selected(context)?.1.target)
    }

    fn contexts_of(&self, callable: LinkTarget) -> impl Iterator<Item = &P::Key> {
        let transfer = self.engine.driver.inner();
        self.envs_of(callable)
            .filter_map(|env| transfer.env.key(env))
    }

    fn fact(&self, context: &P::Key, value: SSAValue) -> Option<&V> {
        let (env, _) = self.engine.selected(context)?;
        let transfer = self.engine.driver.inner();
        transfer.env.environment(env).ok()?.get(value)
    }

    fn facts(&self, context: &P::Key) -> impl Iterator<Item = (SSAValue, &V)> {
        let transfer = self.engine.driver.inner();
        self.engine
            .selected(context)
            .and_then(|(env, _)| transfer.env.environment(env).ok())
            .into_iter()
            .flat_map(|facts| facts.iter())
    }

    /// The join of every reached return; `None` if no return was reached.
    fn response(&self, context: &P::Key) -> Option<Product<V>> {
        self.engine.selected(context)?.1.ret.clone()
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
