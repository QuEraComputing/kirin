//! The forward lattice-based abstract interpreter.
//!
//! # Layering (owner-summary fixpoint)
//!
//! [`SparseForwardInterpreter`] is the public engine. Internally it is a thin
//! wrapper over a [`StandardFixpointInterpreter`] driving a summary-free
//! [`SparseForwardTransfer`]:
//!
//! - **[`SparseForwardTransfer`]** is the [`Interp`] delegate: pipeline, linker, SSA
//!   env, analysis policy, per-function return accumulator, and read/write logging;
//!   it provides the dialect-dispatch / IR-query surface ([`StatementDispatch`],
//!   [`BlockQueries`], [`CFGQueries`], [`DiGraphQueries`], and — for
//!   concrete-shaped callers — [`CallServices`]).
//! - the **[`StandardFixpointInterpreter`]** driver owns the summaries, the
//!   dependency graph ([`ForwardSummaryDeps`]), the owner worklist, and the
//!   owner-local [`ForwardStore`] (shared envs + context-qualified value-reader
//!   deps).
//!
//! # Owner kinds
//!
//! [`Owner::Function`] is a **summary/storage** owner — it is *never scheduled*; it
//! records a function context's entry/return/entry-block. [`Owner::Block`] and
//! [`Owner::Graph`] are the **executable** owners: exactly those run frames (one
//! single-pass walk each — a CFG block, or a whole graph body in dependency
//! order). CFG convergence is owner-summary convergence: a block emits its
//! successor block-entries, its function return, its outputs, and its external
//! read dependencies through the single [`apply_update`](ForwardDriver::apply_update)
//! path, which merges via the analysis policy and reschedules owners / value
//! readers. Direct dominated cross-block SSA uses are tracked by context-qualified
//! [`ValueFactKey`] value-reader deps, so a reader block reruns when a value it
//! read directly rises.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::hash::Hash;
use std::marker::PhantomData;

use kirin_ir::{
    Block, CFG, CompileStage, DiGraph, HasBottom, Pipeline, Product, SSAValue, SpecializedFunction,
    StageMeta, Statement, Symbol, Widen,
};

use crate::core::query;
use crate::{
    AbstractBlockFrame, AbstractCompletion, AbstractDiGraphFrame, AbstractInterpreter,
    BlockQueries, Body, CFGQueries, CallEffect, CallServices, CallableBody, Callee, DiGraphQueries,
    Env, EnvIndex, EnvStackStore, FixpointProfile, ForwardDataflowFrameEngine, ForwardEval,
    ForwardSummaryDeps, Frame, FunctionTarget, Interp, InterpDispatch, InterpLocation,
    InterpreterError, Linker, OwnerSemantics, SameStageLinker, SparseForwardEffect,
    SparseForwardSemantic, StageQuery, StandardAbstractFrame, StandardFixpointInterpreter,
    StatementDispatch, Store, Summary, SummaryDependency, SummaryDependencyIndex, SummaryEffect,
};

// ===========================================================================
// Pluggable analysis seams (policy `P`)
// ===========================================================================

/// Summary-key strategy: maps a resolved call target plus its abstract arguments
/// to the key under which that function's entry/return summary is tracked.
pub trait CallContext<V> {
    type Key: Clone + Eq + Hash;

    fn key(&mut self, target: &FunctionTarget, args: &Product<V>) -> Self::Key;
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

impl<V> CallContext<V> for ContextInsensitive {
    type Key = (CompileStage, SpecializedFunction);

    fn key(&mut self, target: &FunctionTarget, _args: &Product<V>) -> Self::Key {
        (target.stage, target.function)
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
// Owners + summaries
// ===========================================================================

/// Owner of a summary in the forward fixpoint.
///
/// [`Owner::Function`] is a **summary/storage** owner (never scheduled);
/// [`Owner::Block`] and [`Owner::Graph`] are the **executable** owners
/// (frame-executed) — one per unit of re-analysis. `Owner` is a
/// dataflow-equation identity — deliberately **not** a
/// [`LatticeAnchor`](crate::LatticeAnchor).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Owner<K> {
    /// A function summary/storage owner (not frame-executed).
    Function(K),
    /// A CFG block executable owner within function context `K`.
    Block { function: K, block: Block },
    /// A graph-body executable owner within function context `K`: the whole
    /// graph is one unit, re-analyzed as a single dependency-ordered pass
    /// whenever its entry product rises. One pass is exact for a DAG, so there
    /// is no intra-graph fixpoint to split into finer owners.
    Graph { function: K, graph: DiGraph },
}

impl<K> Owner<K> {
    /// The function context this owner belongs to.
    pub fn function(&self) -> &K {
        match self {
            Owner::Function(function)
            | Owner::Block { function, .. }
            | Owner::Graph { function, .. } => function,
        }
    }
}

/// Per-function summary/storage record: call-site metadata, the joined entry
/// arguments, the entry block, and the joined return product.
#[derive(Clone)]
pub struct FunctionSummary<V> {
    /// `(stage, body)` — set when the owner is first seeded from a call site.
    meta: Option<(CompileStage, Statement)>,
    entry: Product<V>,
    entry_joins: usize,
    ret: Option<Product<V>>,
    /// The function's entry block (resolved when the entry is first seeded).
    entry_block: Option<Block>,
}

impl<V> FunctionSummary<V> {
    fn bottom() -> Self {
        Self {
            meta: None,
            entry: Product::new(),
            entry_joins: 0,
            ret: None,
            entry_block: None,
        }
    }
}

/// Per-block summary: the joined block-entry (parameter) product plus the block's
/// output facts (the abstract values it defines, read elsewhere).
#[derive(Clone)]
pub struct BlockSummary<V> {
    entry: Product<V>,
    entry_joins: usize,
    outputs: HashMap<SSAValue, V>,
}

impl<V> BlockSummary<V> {
    fn bottom() -> Self {
        Self {
            entry: Product::new(),
            entry_joins: 0,
            outputs: HashMap::new(),
        }
    }
}

/// The unified per-owner summary the driver stores (its `summaries` map is keyed
/// by a single [`Owner`]).
///
/// [`Summary::merge`] is inert: the forward engine merges entry/return/block facts
/// through the analysis policy directly in
/// [`apply_update`](ForwardDriver::apply_update), so the driver never emits
/// [`SummaryEffect::Update`] for these summaries.
#[derive(Clone)]
pub enum ForwardSummary<V> {
    /// A function context's entry/return record.
    Function(FunctionSummary<V>),
    /// A block's entry/output summary.
    Block(BlockSummary<V>),
}

impl<V> ForwardSummary<V> {
    fn as_function(&self) -> Option<&FunctionSummary<V>> {
        match self {
            ForwardSummary::Function(function) => Some(function),
            ForwardSummary::Block(_) => None,
        }
    }

    fn as_function_mut(&mut self) -> Option<&mut FunctionSummary<V>> {
        match self {
            ForwardSummary::Function(function) => Some(function),
            ForwardSummary::Block(_) => None,
        }
    }

    fn as_block(&self) -> Option<&BlockSummary<V>> {
        match self {
            ForwardSummary::Block(block) => Some(block),
            ForwardSummary::Function(_) => None,
        }
    }

    fn as_block_mut(&mut self) -> Option<&mut BlockSummary<V>> {
        match self {
            ForwardSummary::Block(block) => Some(block),
            ForwardSummary::Function(_) => None,
        }
    }
}

impl<V: Clone> Summary for ForwardSummary<V> {
    type Strategy = ();
    type Change = ();

    fn merge(
        &mut self,
        _phase: crate::FixpointPhase,
        _candidate: Self,
        _strategy: &mut Self::Strategy,
    ) -> Option<Self::Change> {
        None
    }
}

/// Context-qualified key for value-reader dependencies: the same [`SSAValue`] under
/// two different function contexts is two distinct facts, so readers never
/// cross-contaminate.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ValueFactKey<K> {
    pub function: K,
    pub value: SSAValue,
}

/// Owner-local analysis state carried in the driver's `store`: one shared env per
/// function context (so direct dominated cross-block uses resolve), plus the
/// context-qualified value-reader dependency index. **Not** part of the public
/// function-summary surface.
pub struct ForwardStore<K, V> {
    envs: HashMap<K, EnvIndex>,
    value_readers: HashMap<ValueFactKey<K>, HashSet<Owner<K>>>,
    _marker: PhantomData<fn() -> V>,
}

impl<K, V> ForwardStore<K, V> {
    fn new() -> Self {
        Self {
            envs: HashMap::new(),
            value_readers: HashMap::new(),
            _marker: PhantomData,
        }
    }
}

impl<K: Clone + Eq + Hash, V> ForwardStore<K, V> {
    fn env(&self, function: &K) -> Option<EnvIndex> {
        self.envs.get(function).copied()
    }

    fn set_env(&mut self, function: K, index: EnvIndex) {
        self.envs.insert(function, index);
    }

    fn register_reader(&mut self, key: ValueFactKey<K>, reader: Owner<K>) {
        self.value_readers.entry(key).or_default().insert(reader);
    }

    fn readers_of(&self, key: &ValueFactKey<K>) -> Vec<Owner<K>> {
        self.value_readers
            .get(key)
            .map(|readers| readers.iter().cloned().collect())
            .unwrap_or_default()
    }
}

/// A single mutation the driver applies through [`apply_update`](ForwardDriver::apply_update).
enum ForwardUpdate<K, V> {
    /// Merge call args into a function context's entry (widen by visits); on rise,
    /// (re)seed its entry block.
    FunctionEntry {
        key: K,
        stage: CompileStage,
        definition: Statement,
        args: Product<V>,
    },
    /// Merge a return contribution into a function context's return (join); on
    /// rise, reschedule its callers.
    FunctionReturn { key: K, values: Product<V> },
    /// Merge incoming args into an **executable** owner's entry (widen by
    /// visits); on rise, (re)schedule that owner. The incoming args are a CFG
    /// edge's arguments for a block owner, or the boundary-port values for a
    /// graph owner.
    OwnerEntry { owner: Owner<K>, args: Product<V> },
    /// Merge an owner's freshly computed outputs (join); on any value's rise,
    /// reschedule that value's readers.
    OwnerOutputs {
        owner: Owner<K>,
        outputs: HashMap<SSAValue, V>,
    },
}

/// The owner-summary type family bundling the forward summaries onto the driver.
pub struct SparseForwardProfile<V, E, K, F> {
    #[allow(clippy::type_complexity)]
    _marker: PhantomData<fn() -> (V, E, K, F)>,
}

impl<'ir, S, V, E, Lk, P, F, Sem>
    FixpointProfile<SparseForwardTransfer<'ir, S, V, E, Lk, P, F, Sem>>
    for SparseForwardProfile<V, E, <P as CallContext<V>>::Key, F>
where
    S: StageMeta,
    V: Clone + HasBottom,
    E: From<InterpreterError>,
    P: CallContext<V>,
    Sem: SparseForwardSemantic,
{
    type SummaryKey = Owner<<P as CallContext<V>>::Key>;
    type Summary = ForwardSummary<V>;
    type Frame = F;
    type Completion = AbstractCompletion<V>;
}

/// The forward driver: a [`StandardFixpointInterpreter`] over [`SparseForwardTransfer`]
/// with owner summaries, forward dependencies, and the owner-local
/// [`ForwardStore`].
type ForwardDriver<'ir, S, V, E, Lk, P, F, Sem> = StandardFixpointInterpreter<
    SparseForwardTransfer<'ir, S, V, E, Lk, P, F, Sem>,
    SparseForwardProfile<V, E, <P as CallContext<V>>::Key, F>,
    ForwardStore<<P as CallContext<V>>::Key, V>,
    ForwardSummaryDeps<Owner<<P as CallContext<V>>::Key>>,
>;

// ===========================================================================
// SparseForwardTransfer — the summary-free Interp delegate
// ===========================================================================

/// The summary-free transfer + env of the forward abstract interpreter.
pub struct SparseForwardTransfer<
    'ir,
    S: StageMeta,
    V,
    E,
    Lk = SameStageLinker,
    P = ContextInsensitive,
    F = StandardAbstractFrame<V, E, <P as CallContext<V>>::Key>,
    Sem = ForwardEval,
> where
    P: CallContext<V>,
    Sem: SparseForwardSemantic,
{
    pipeline: &'ir Pipeline<S>,
    linker: Lk,
    store: EnvStackStore<V>,
    analysis: P,
    max_iterations: usize,
    location: Option<InterpLocation>,
    ret_acc: Option<Product<V>>,
    /// SSA values read during the current block-owner walk (interior-mutable
    /// because [`Env::env_read`] takes `&self`).
    read_log: RefCell<Vec<SSAValue>>,
    /// SSA values written during the current block-owner walk.
    write_log: Vec<SSAValue>,
    /// Whether read/write logging is active (only during a block-owner walk).
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
        Self {
            pipeline,
            linker: SameStageLinker,
            store: EnvStackStore::new(),
            analysis: P::default(),
            max_iterations: 1000,
            location: None,
            ret_acc: None,
            read_log: RefCell::new(Vec::new()),
            write_log: Vec::new(),
            logging: false,
            _marker: PhantomData,
        }
    }
}

impl<'ir, S: StageMeta, V, E, Lk, P, F, Sem> SparseForwardTransfer<'ir, S, V, E, Lk, P, F, Sem>
where
    P: CallContext<V>,
    Sem: SparseForwardSemantic,
{
    fn with_linker<Lk2>(self, linker: Lk2) -> SparseForwardTransfer<'ir, S, V, E, Lk2, P, F, Sem> {
        SparseForwardTransfer {
            pipeline: self.pipeline,
            linker,
            store: self.store,
            analysis: self.analysis,
            max_iterations: self.max_iterations,
            location: self.location,
            ret_acc: self.ret_acc,
            read_log: self.read_log,
            write_log: self.write_log,
            logging: self.logging,
            _marker: PhantomData,
        }
    }

    #[allow(clippy::type_complexity)]
    fn with_analysis<P2>(
        self,
        analysis: P2,
    ) -> SparseForwardTransfer<
        'ir,
        S,
        V,
        E,
        Lk,
        P2,
        StandardAbstractFrame<V, E, <P2 as CallContext<V>>::Key>,
        Sem,
    >
    where
        P2: CallContext<V>,
    {
        SparseForwardTransfer {
            pipeline: self.pipeline,
            linker: self.linker,
            store: EnvStackStore::new(),
            analysis,
            max_iterations: self.max_iterations,
            location: None,
            ret_acc: None,
            read_log: RefCell::new(Vec::new()),
            write_log: Vec::new(),
            logging: false,
            _marker: PhantomData,
        }
    }

    fn set_analysis(&mut self, analysis: P) {
        self.analysis = analysis;
    }

    fn pipeline(&self) -> &'ir Pipeline<S> {
        self.pipeline
    }

    /// Begin logging reads/writes for a block-owner walk.
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

// Policy-driven merge + return accumulation, kept on the transfer (the analysis `P`
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
    fn key(&mut self, target: &FunctionTarget, args: &Product<V>) -> <P as CallContext<V>>::Key {
        self.analysis.key(target, args)
    }

    fn take_ret_acc(&mut self) -> Option<Product<V>> {
        self.ret_acc.take()
    }

    /// Join `incoming` into the return accumulator (never widens).
    fn contribute_return(&mut self, incoming: Product<V>) -> Result<(), E> {
        match &self.ret_acc {
            None => self.ret_acc = Some(incoming),
            Some(current) => {
                self.ret_acc = Some(
                    self.analysis
                        .merge(current, &incoming, 0)
                        .map_err(E::from)?,
                );
            }
        }
        Ok(())
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
    fn env_read(&self, index: EnvIndex, value: SSAValue) -> Result<V, E> {
        // Log the read regardless of whether it resolves to a bound value or
        // bottom — an unbound read of a value defined elsewhere is exactly the
        // cross-block dependency to track.
        if self.logging {
            self.read_log.borrow_mut().push(value);
        }
        match self.store.read(index, value) {
            Ok(value) => Ok(value),
            Err(InterpreterError::UnboundValue { .. }) => Ok(V::bottom()),
            Err(error) => Err(E::from(error)),
        }
    }

    fn env_write(&mut self, index: EnvIndex, value: SSAValue, data: V) -> Result<(), E> {
        if self.logging {
            self.write_log.push(value);
        }
        self.store.write(index, value, data).map_err(E::from)
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
        self.store.alloc()
    }

    fn free_env(&mut self, index: EnvIndex) -> Result<(), E> {
        self.store.free(index).map_err(E::from)
    }

    fn resolve_call(&self, stage: CompileStage, callee: &Callee) -> Result<FunctionTarget, E> {
        self.linker
            .resolve(self.pipeline, stage, callee)
            .map_err(E::from)
    }

    fn enter_function(
        &mut self,
        stage: CompileStage,
        definition: Statement,
        args: Product<V>,
        index: EnvIndex,
    ) -> Result<CallableBody<V>, E> {
        let pipeline = self.pipeline;
        let info = pipeline
            .stage(stage)
            .ok_or_else(|| E::from(InterpreterError::MissingStage(stage)))?;
        let previous = self.location.replace(InterpLocation {
            stage,
            statement: definition,
            index,
        });
        let result = info.dispatch_function_entry(definition, args, self);
        self.location = previous;
        result
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

    fn resolve_call(&self, stage: CompileStage, callee: &Callee) -> Result<FunctionTarget, E> {
        self.inner().resolve_call(stage, callee)
    }

    fn enter_function(
        &mut self,
        stage: CompileStage,
        definition: Statement,
        args: Product<V>,
        index: EnvIndex,
    ) -> Result<CallableBody<V>, E> {
        self.inner_mut()
            .enter_function(stage, definition, args, index)
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
    type SummaryKey = <P as CallContext<V>>::Key;

    fn analysis_merge(
        &self,
        current: &Product<V>,
        incoming: &Product<V>,
        visits: usize,
    ) -> Result<Product<V>, E> {
        self.inner().merge_products(current, incoming, visits)
    }

    fn contribute_return(&mut self, values: Product<V>) -> Result<(), E> {
        self.inner_mut().contribute_return(values)
    }

    fn current_function_key(&self) -> Option<<P as CallContext<V>>::Key> {
        self.current_owner().map(|owner| owner.function().clone())
    }

    /// Summarize a call atomically: resolve, merge the callee entry (which seeds
    /// its entry block) through [`apply_update`](Self::apply_update), register the
    /// caller dependency (including same-key self-recursion), and read the callee's
    /// current return (or bottom).
    fn summarize_call(
        &mut self,
        stage: CompileStage,
        call: CallEffect<V>,
        index: EnvIndex,
    ) -> Result<(), E> {
        let CallEffect {
            callee,
            stage: call_stage,
            args,
            results,
        } = call;
        let resolve_stage = call_stage.unwrap_or(stage);
        let target = self.inner().resolve_call(resolve_stage, &callee)?;
        let key = self.inner_mut().key(&target, &args);

        self.apply_update(ForwardUpdate::FunctionEntry {
            key: key.clone(),
            stage: target.stage,
            definition: target.definition,
            args,
        })?;

        let owner = Owner::Function(key);
        if let Some(caller) = self.current_owner().cloned() {
            self.dependency_index_mut()
                .register(&owner, SummaryDependency::Reanalyze(caller))
                .expect("forward dependency index is infallible");
        }

        let ret = self
            .summary(&owner)
            .and_then(|info| info.as_function())
            .and_then(|function| function.ret.clone());
        match ret {
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

// The single mutation/scheduling path (constraint 5): every summary/fact change
// and every reschedule flows through `apply_update`.
impl<'ir, S, V, E, Lk, P, F, Sem> ForwardDriver<'ir, S, V, E, Lk, P, F, Sem>
where
    S: StageQuery + InterpDispatch<SparseForwardTransfer<'ir, S, V, E, Lk, P, F, Sem>>,
    V: Clone + PartialEq + Widen + HasBottom,
    E: From<InterpreterError>,
    Lk: Linker<S>,
    P: CallContext<V> + WideningStrategy<V>,
    Sem: SparseForwardSemantic,
{
    fn apply_update(
        &mut self,
        update: ForwardUpdate<<P as CallContext<V>>::Key, V>,
    ) -> Result<(), E> {
        match update {
            ForwardUpdate::FunctionEntry {
                key,
                stage,
                definition,
                args,
            } => {
                let owner = Owner::Function(key.clone());
                let changed = if self.summary(&owner).is_none() {
                    self.summaries_mut().insert(
                        owner.clone(),
                        ForwardSummary::Function(FunctionSummary {
                            meta: Some((stage, definition)),
                            entry: args,
                            entry_joins: 0,
                            ret: None,
                            entry_block: None,
                        }),
                    );
                    self.dependency_index_mut()
                        .ensure_owner(&owner)
                        .expect("forward dependency index is infallible");
                    true
                } else {
                    let (old, visits) = {
                        let function = self
                            .summary(&owner)
                            .and_then(|info| info.as_function())
                            .expect("function summary present");
                        (function.entry.clone(), function.entry_joins + 1)
                    };
                    let merged = self.inner().merge_products(&old, &args, visits)?;
                    let changed = merged != old;
                    let function = self
                        .summary_mut(&owner)
                        .and_then(|info| info.as_function_mut())
                        .expect("function summary present");
                    function.entry_joins = visits;
                    if changed {
                        function.entry = merged;
                    }
                    changed
                };
                if changed {
                    self.seed_entry_block(&key, stage, definition)?;
                }
                Ok(())
            }

            ForwardUpdate::FunctionReturn { key, values } => {
                let owner = Owner::Function(key);
                let old = self
                    .summary(&owner)
                    .and_then(|info| info.as_function())
                    .and_then(|function| function.ret.clone());
                let merged = match old {
                    None => values,
                    Some(old) => self.inner().merge_products(&old, &values, 0)?,
                };
                let changed = {
                    let function = self
                        .summary_mut(&owner)
                        .and_then(|info| info.as_function_mut())
                        .ok_or_else(|| {
                            E::from(InterpreterError::Custom("missing function summary"))
                        })?;
                    if function.ret.as_ref() != Some(&merged) {
                        function.ret = Some(merged);
                        true
                    } else {
                        false
                    }
                };
                if changed {
                    let deps = self
                        .dependency_index_mut()
                        .on_summary_changed(&owner, ())
                        .expect("forward dependency index is infallible");
                    for dep in deps {
                        let SummaryDependency::Reanalyze(caller) = dep;
                        self.schedule(caller);
                    }
                }
                Ok(())
            }

            ForwardUpdate::OwnerEntry { owner, args } => {
                let changed = if self.summary(&owner).is_none() {
                    self.summaries_mut().insert(
                        owner.clone(),
                        ForwardSummary::Block(BlockSummary {
                            entry: args,
                            entry_joins: 0,
                            outputs: HashMap::new(),
                        }),
                    );
                    self.dependency_index_mut()
                        .ensure_owner(&owner)
                        .expect("forward dependency index is infallible");
                    true
                } else {
                    let (old, visits) = {
                        let summary = self
                            .summary(&owner)
                            .and_then(|info| info.as_block())
                            .expect("block summary present");
                        (summary.entry.clone(), summary.entry_joins + 1)
                    };
                    let merged = self.inner().merge_products(&old, &args, visits)?;
                    let changed = merged != old;
                    let summary = self
                        .summary_mut(&owner)
                        .and_then(|info| info.as_block_mut())
                        .expect("block summary present");
                    summary.entry_joins = visits;
                    if changed {
                        summary.entry = merged;
                    }
                    changed
                };
                if changed {
                    self.schedule(owner);
                }
                Ok(())
            }

            ForwardUpdate::OwnerOutputs { owner, outputs } => {
                let function = owner.function().clone();
                let mut risen = Vec::new();
                for (value, incoming) in outputs {
                    let old = self
                        .summary(&owner)
                        .and_then(|info| info.as_block())
                        .and_then(|summary| summary.outputs.get(&value).cloned());
                    let merged = match &old {
                        None => incoming,
                        Some(old) => old.join(&incoming),
                    };
                    if old.as_ref() != Some(&merged) {
                        let summary = self
                            .summary_mut(&owner)
                            .and_then(|info| info.as_block_mut())
                            .expect("block summary present");
                        summary.outputs.insert(value, merged);
                        risen.push(value);
                    }
                }
                for value in risen {
                    let readers = self.store().readers_of(&ValueFactKey {
                        function: function.clone(),
                        value,
                    });
                    for reader in readers {
                        self.schedule(reader);
                    }
                }
                Ok(())
            }
        }
    }

    /// Resolve the executable entry owner of `key`'s function (allocating its
    /// shared env on first use) and seed it with the entry arguments.
    ///
    /// This is the one place a *function* becomes runnable *work*: it translates
    /// the callable [`Body`] into the executable [`Owner`] the worklist can
    /// hold — a `CFG`'s entry block or a `Block` body become an
    /// [`Owner::Block`], a `DiGraph` body becomes an [`Owner::Graph`]. An
    /// `UnGraph` has no derivable traversal order at all, so it is rejected.
    fn seed_entry_block(
        &mut self,
        key: &<P as CallContext<V>>::Key,
        stage: CompileStage,
        definition: Statement,
    ) -> Result<(), E> {
        let env = match self.store().env(key) {
            Some(env) => env,
            None => {
                let env = self.alloc_env();
                self.store_mut().set_env(key.clone(), env);
                env
            }
        };
        let entry_args = self
            .summary(&Owner::Function(key.clone()))
            .and_then(|info| info.as_function())
            .map(|function| function.entry.clone())
            .expect("function summary present");
        let entry = self.enter_function(stage, definition, entry_args, env)?;
        let owner = match entry.body {
            Body::CFG(cfg) => Owner::Block {
                function: key.clone(),
                block: self
                    .cfg_entry(stage, cfg)?
                    .ok_or_else(|| E::from(InterpreterError::EmptyCFG))?,
            },
            Body::Block(block) => Owner::Block {
                function: key.clone(),
                block,
            },
            // A graph body has no blocks: it is its own unit of re-analysis.
            Body::DiGraph(graph) => Owner::Graph {
                function: key.clone(),
                graph,
            },
            // An undirected graph has no producer/consumer direction, so no
            // traversal order can be derived from its structure.
            graph @ Body::UnGraph(_) => {
                return Err(E::from(InterpreterError::NoDefaultWalker(graph)));
            }
        };
        if let Some(function) = self
            .summary_mut(&Owner::Function(key.clone()))
            .and_then(|info| info.as_function_mut())
        {
            function.entry_block = match &owner {
                Owner::Block { block, .. } => Some(*block),
                _ => None,
            };
        }
        self.apply_update(ForwardUpdate::OwnerEntry {
            owner,
            args: entry.args,
        })
    }
}

// ===========================================================================
// Owner semantics: block and graph owners are executable.
// ===========================================================================

/// The forward owner semantics. [`Owner::Block`] and [`Owner::Graph`] owners are
/// analyzed: bind the entry product, walk the unit once, then route its outputs /
/// successor edges / return / read-deps through
/// [`apply_update`](ForwardDriver::apply_update). A graph owner has no successor
/// edges — its declared yields are the function's return instead.
struct SparseForwardSemantics<V> {
    _marker: PhantomData<fn() -> V>,
}

impl<V> SparseForwardSemantics<V> {
    fn new() -> Self {
        Self {
            _marker: PhantomData,
        }
    }
}

impl<'ir, S, V, E, Lk, P, F, Sem>
    OwnerSemantics<
        ForwardDriver<'ir, S, V, E, Lk, P, F, Sem>,
        Owner<<P as CallContext<V>>::Key>,
        ForwardSummary<V>,
        F,
        AbstractCompletion<V>,
        E,
    > for SparseForwardSemantics<V>
where
    S: StageQuery + InterpDispatch<SparseForwardTransfer<'ir, S, V, E, Lk, P, F, Sem>>,
    V: Clone + PartialEq + Widen + HasBottom,
    E: From<InterpreterError>,
    Lk: Linker<S>,
    P: CallContext<V> + WideningStrategy<V>,
    Sem: SparseForwardSemantic,
    F: From<AbstractBlockFrame<V, E, <P as CallContext<V>>::Key>>
        + From<AbstractDiGraphFrame<V, E, <P as CallContext<V>>::Key>>,
{
    fn bottom_summary(
        &mut self,
        _interp: &mut ForwardDriver<'ir, S, V, E, Lk, P, F, Sem>,
        owner: &Owner<<P as CallContext<V>>::Key>,
    ) -> Result<ForwardSummary<V>, E> {
        // `apply_update` seeds real summaries before scheduling; this is only a
        // safe default for the dependency-index bookkeeping path.
        Ok(match owner {
            Owner::Function(_) => ForwardSummary::Function(FunctionSummary::bottom()),
            // Both executable owners carry the same shape of summary: a joined
            // entry product plus the output facts they define.
            Owner::Block { .. } | Owner::Graph { .. } => {
                ForwardSummary::Block(BlockSummary::bottom())
            }
        })
    }

    fn entry_frame(
        &mut self,
        interp: &mut ForwardDriver<'ir, S, V, E, Lk, P, F, Sem>,
        owner: &Owner<<P as CallContext<V>>::Key>,
        summary: &ForwardSummary<V>,
    ) -> Result<F, E> {
        let function = match owner {
            Owner::Block { function, .. } | Owner::Graph { function, .. } => function.clone(),
            Owner::Function(_) => {
                return Err(E::from(InterpreterError::Custom(
                    "function owners are storage-only and never executed",
                )));
            }
        };
        let block_entry = summary
            .as_block()
            .ok_or_else(|| {
                E::from(InterpreterError::Custom(
                    "block owner analyzed with a non-block summary",
                ))
            })?
            .entry
            .clone();
        let stage = interp
            .summary(&Owner::Function(function.clone()))
            .and_then(|info| info.as_function())
            .and_then(|f| f.meta)
            .map(|(stage, _)| stage)
            .ok_or_else(|| {
                E::from(InterpreterError::Custom(
                    "block owner's function is unseeded",
                ))
            })?;
        let env = interp.store().env(&function).ok_or_else(|| {
            E::from(InterpreterError::Custom(
                "block owner's function has no shared env",
            ))
        })?;
        interp.inner_mut().begin_block_log();
        match owner {
            Owner::Block { block, .. } => {
                Ok(AbstractBlockFrame::new_cfg_block(stage, env, *block, block_entry).into())
            }
            // One dependency-ordered pass over the whole graph. Exact for a DAG,
            // so the pass never needs to iterate internally.
            Owner::Graph { graph, .. } => {
                Ok(AbstractDiGraphFrame::new(stage, env, *graph, block_entry).into())
            }
            Owner::Function(_) => Err(E::from(InterpreterError::Custom(
                "function owners are storage-only and never executed",
            ))),
        }
    }

    fn complete_owner(
        &mut self,
        interp: &mut ForwardDriver<'ir, S, V, E, Lk, P, F, Sem>,
        owner: Owner<<P as CallContext<V>>::Key>,
        completion: AbstractCompletion<V>,
    ) -> Result<SummaryEffect<Owner<<P as CallContext<V>>::Key>, ForwardSummary<V>>, E> {
        let function = match &owner {
            Owner::Block { function, .. } | Owner::Graph { function, .. } => function.clone(),
            Owner::Function(_) => {
                return Err(E::from(InterpreterError::Custom(
                    "function owners are storage-only and never executed",
                )));
            }
        };
        // A block owner completes with its outgoing CFG edges. A graph owner has
        // no successors at all — it completes with the graph's declared yields,
        // which for a callable graph body *are* the function's return values.
        let (edges, graph_yields) = match (&owner, completion) {
            (Owner::Block { .. }, AbstractCompletion::CFGBlock { edges }) => (edges, None),
            (Owner::Graph { .. }, AbstractCompletion::Finished(values)) => (Vec::new(), values),
            _ => {
                return Err(E::from(InterpreterError::Custom(
                    "executable owner completed with a mismatched completion",
                )));
            }
        };
        if let Some(values) = graph_yields {
            interp.contribute_return(values)?;
        }

        let (reads, writes) = interp.inner_mut().take_logs();
        let env = interp.store().env(&function).ok_or_else(|| {
            E::from(InterpreterError::Custom(
                "block owner's function has no shared env",
            ))
        })?;

        // Register external direct reads (values read but not written locally) as
        // context-qualified value-reader deps on this block owner.
        let written: HashSet<SSAValue> = writes.iter().copied().collect();
        for value in reads {
            if !written.contains(&value) {
                interp.store_mut().register_reader(
                    ValueFactKey {
                        function: function.clone(),
                        value,
                    },
                    owner.clone(),
                );
            }
        }

        // Capture this block's output facts from the shared env (logging is now
        // off, so these reads are not re-logged).
        let mut outputs = HashMap::new();
        for value in writes {
            let fact = interp.inner().env_read(env, value)?;
            outputs.insert(value, fact);
        }
        interp.apply_update(ForwardUpdate::OwnerOutputs {
            owner: owner.clone(),
            outputs,
        })?;

        // Propagate CFG successor edges as block-owner entry updates. Empty for a
        // returning block and for a graph owner.
        for edge in edges {
            interp.apply_update(ForwardUpdate::OwnerEntry {
                owner: Owner::Block {
                    function: function.clone(),
                    block: edge.target,
                },
                args: edge.args,
            })?;
        }

        // Flush any return this block contributed into the function return.
        if let Some(values) = interp.inner_mut().take_ret_acc() {
            interp.apply_update(ForwardUpdate::FunctionReturn {
                key: function,
                values,
            })?;
        }

        Ok(SummaryEffect::None)
    }
}

// ===========================================================================
// The public engine: a wrapper over the driver
// ===========================================================================

/// Forward lattice-based abstract interpreter.
///
/// Drives the same forward dialect rules (`Interpretable<I, ForwardEval>`) and
/// [`SparseForwardEffect`] as concrete execution, over an abstract value domain.
/// Traversal is owned by the total frame type `F`; summary keying and merge/widen
/// behavior are owned by the policy `P`.
///
/// ```ignore
/// let mut analysis = SparseForwardInterpreter::<Stage, ConstPropValue, MyError>::new(&pipeline)
///     .with_linker(CrossStageLinker);
/// let result = analysis.analyze_by_name("source", "abs", [ConstPropValue::Const(7)])?;
/// ```
pub struct SparseForwardInterpreter<
    'ir,
    S: StageMeta,
    V,
    E,
    Lk = SameStageLinker,
    P = ContextInsensitive,
    F = StandardAbstractFrame<V, E, <P as CallContext<V>>::Key>,
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
            driver: StandardFixpointInterpreter::with_dependency_index(
                SparseForwardTransfer::new(pipeline),
                ForwardStore::new(),
                (),
                ForwardSummaryDeps::new(),
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
    /// type `F`; resets the (empty) summary tables.
    pub fn with_linker<Lk2>(
        self,
        linker: Lk2,
    ) -> SparseForwardInterpreter<'ir, S, V, E, Lk2, P, F, Sem> {
        let transfer = self.driver.into_inner().with_linker(linker);
        SparseForwardInterpreter {
            driver: StandardFixpointInterpreter::with_dependency_index(
                transfer,
                ForwardStore::new(),
                (),
                ForwardSummaryDeps::new(),
            ),
        }
    }

    /// Swap the analysis policy (context abstraction + join/widen). Changes the
    /// [`CallContext::Key`] type, so this resets the summary tables and frame type.
    #[allow(clippy::type_complexity)]
    pub fn with_analysis<P2>(
        self,
        analysis: P2,
    ) -> SparseForwardInterpreter<
        'ir,
        S,
        V,
        E,
        Lk,
        P2,
        StandardAbstractFrame<V, E, <P2 as CallContext<V>>::Key>,
        Sem,
    >
    where
        P2: CallContext<V>,
    {
        let transfer = self.driver.into_inner().with_analysis(analysis);
        SparseForwardInterpreter {
            driver: StandardFixpointInterpreter::with_dependency_index(
                transfer,
                ForwardStore::new(),
                (),
                ForwardSummaryDeps::new(),
            ),
        }
    }

    /// Replace the analysis policy *value* while keeping its type (and so the
    /// [`CallContext::Key`] and the frame type `F`).
    pub fn with_policy(self, analysis: P) -> Self {
        let mut transfer = self.driver.into_inner();
        transfer.set_analysis(analysis);
        Self {
            driver: StandardFixpointInterpreter::with_dependency_index(
                transfer,
                ForwardStore::new(),
                (),
                ForwardSummaryDeps::new(),
            ),
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

    /// Inspect the return summary of an analyzed function (context-insensitive
    /// keying only).
    pub fn return_summary(
        &self,
        stage: CompileStage,
        function: SpecializedFunction,
    ) -> Option<&Product<V>> {
        self.driver
            .summary(&Owner::Function((stage, function)))
            .and_then(|info| info.as_function())
            .and_then(|function| function.ret.as_ref())
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
        + From<AbstractBlockFrame<V, E, <P as CallContext<V>>::Key>>
        + From<AbstractDiGraphFrame<V, E, <P as CallContext<V>>::Key>>,
{
    /// Resolve `stage`/`function` by name and analyze. Returns the function's
    /// inferred return product at the fixpoint (empty if it never returns).
    pub fn analyze_by_name(
        &mut self,
        stage_name: &str,
        function_name: &str,
        args: impl IntoIterator<Item = V>,
    ) -> Result<Product<V>, E> {
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
        &mut self,
        stage: CompileStage,
        symbol: Symbol,
        args: impl IntoIterator<Item = V>,
    ) -> Result<Product<V>, E> {
        self.analyze(stage, symbol.into(), args)
    }

    /// Run the fixpoint from a single entry. Seeds the entry function's entry block
    /// owner and drains the owner worklist.
    pub fn analyze(
        &mut self,
        stage: CompileStage,
        callee: Callee,
        args: impl IntoIterator<Item = V>,
    ) -> Result<Product<V>, E> {
        let target = self.driver.inner().resolve_call(stage, &callee)?;
        let args: Product<V> = args.into_iter().collect();
        let key = self.driver.inner_mut().key(&target, &args);

        self.driver.apply_update(ForwardUpdate::FunctionEntry {
            key: key.clone(),
            stage: target.stage,
            definition: target.definition,
            args,
        })?;

        // TODO: Rename this, "semantics" is a bit misleading, sounds like the
        // Semantic Keys, e.g. ForwardEval.
        // Alternative: Rename the keys to: SparseForwardKey / SparseBackwardKey / DenseBackwardKey.
        let mut semantics = SparseForwardSemantics::new();
        self.driver.drain_worklist(&mut semantics)?;

        Ok(self
            .driver
            .summary(&Owner::Function(key))
            .and_then(|info| info.as_function())
            .and_then(|function| function.ret.clone())
            .unwrap_or_default())
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
