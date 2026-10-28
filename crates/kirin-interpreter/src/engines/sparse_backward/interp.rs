//! The sparse backward demand engine (strong liveness / neededness).
//!
//! # The analysis
//!
//! A **sparse backward** fact is one lattice element per SSA value: *how
//! strongly is this value needed?* For strong (true) liveness the lattice is
//! two-point (demanded or not): a value is demanded iff it transitively feeds
//! a *root* — a requested output, a control root (a branch condition, loop
//! bounds), or an impure ordinary statement. Dead code demands nothing. The
//! fact does not vary by program point, so it anchors to [`SSAValue`]s;
//! per-point live sets are the dense analysis's product (`ClassicLiveness`),
//! not this one's.
//!
//! # Layering (owner-summary fixpoint)
//!
//! [`SparseBackwardInterpreter`] is the public engine. Internally it is a
//! wrapper over a [`StandardFixpointInterpreter`] driving a summary-free
//! [`SparseBackwardTransfer`]:
//!
//! - **[`SparseBackwardTransfer`]** is the [`Interp`] delegate: pipeline
//!   access, the calling convention and context policy, the real dispatch
//!   location and callable context, and the per-rule buffers.
//! - the **[`StandardFixpointInterpreter`]** driver owns the demand facts
//!   (summaries keyed by context-qualified SSA values), the worklist, and
//!   the discovered callable contexts.
//!
//! # Owners are fact locations; scheduling is demand propagation
//!
//! `SummaryKey = Scoped<K, SSAValue>`, where `K` is the callable context a
//! [`DemandContext`] policy selects (default: the resolved [`LinkTarget`](crate::LinkTarget)).
//! The fact location is the owner. The driver's *default* self-dependent
//! index ([`OwnerSummaryDeps`]) is exactly demand propagation — a location
//! whose fact rises is rescheduled, and analyzing it means dispatching, in its
//! context, the rules that translate its demand:
//!
//! - a statement **result** → the defining statement's backward rule;
//! - a **block argument** → its directly owning structured statement, or each
//!   indexed CFG predecessor block's terminator;
//! - a graph **port** → the statement owning the graph boundary.
//!
//! Requested outputs merge directly into the operands of the context's return
//! sites (declared through [`declare_return`](SparseBackwardInterp::declare_return)).
//!
//! Call rules cross into another context through
//! [`summarize_call`](SparseBackwardInterp::summarize_call), which registers the one
//! non-topological dependency: callee parameter facts → the caller's call
//! work. All fact mutation flows through the driver's single merge path.
//! Facts only rise in a finite-height lattice and the default context space
//! is bounded, so one shared worklist converges — recursion included — with no
//! nested fixpoints, no widening, and no frames for structured control.

use std::collections::{HashMap, HashSet, VecDeque};
use std::hash::Hash;
use std::marker::PhantomData;
use std::mem;

use kirin_ir::{
    Block, CompileStage, HasArguments, HasBottom, HasResults, HasTop, IsPure, Lattice, Pipeline,
    Product, SSAKind, SSAValue, StageMeta, Statement, Symbol,
};

use super::DemandFrame;
use crate::core::query;
use crate::{
    AbstractInterpreter, Body, Callee, ContextInsensitive, EnvIndex, FixpointProfile, Interp,
    InterpDispatch, InterpLocation, InterpreterError, LinkTarget, Linker, OwnerAnalysis,
    OwnerSummaryDeps, SameStageLinker, Scoped, SparseBackwardSemantic, SparseStore, StageQuery,
    StandardFixpointInterpreter, StrongDemand, Summary, SummaryDependency, SummaryDependencyIndex,
    SummaryEffect, TerminatorArgs,
};

/// A resolved `(stage, body)` pair: the structure one callable context
/// analyzes (and the scope dense liveness qualifies its facts with).
///
/// Arena ids are per-stage, so the stage is part of the scope.
pub type BodyScope = (CompileStage, Body);

// ===========================================================================
// Pluggable analysis seams (policy `P`)
// ===========================================================================

/// Context-selection policy for demand: which callable context tracks the
/// demand of a resolved call target.
///
/// A policy must select stable instances from a bounded space. Equal keys
/// must name the same target. Requests carry boundary facts, not a call-site
/// identity.
pub trait DemandContext<V> {
    type Key: Clone + Eq + Hash;

    fn key(&mut self, target: &LinkTarget, outputs: &Product<V>) -> Self::Key;
}

/// One context per resolved callable, regardless of output request.
impl<V> DemandContext<V> for ContextInsensitive {
    type Key = LinkTarget;

    fn key(&mut self, target: &LinkTarget, _outputs: &Product<V>) -> LinkTarget {
        *target
    }
}

// ===========================================================================
// Effect + dialect-facing trait
// ===========================================================================

/// What a backward rule produced: the local demand facts it raised.
///
/// Rules raise facts imperatively through
/// [`raise_fact`](SparseBackwardInterp::raise_fact) (or its [`DemandInterp`]
/// spelling [`demand`](DemandInterp::demand)) and finish with
/// [`effect()`](SparseBackwardInterp::effect), which drains the buffer into
/// this value; the driver qualifies the values with the rule's callable
/// context and applies them through its single merge path.
pub enum SparseBackwardEffect<V> {
    /// Demand facts to merge (possibly empty).
    Demands(Vec<(SSAValue, V)>),
}

/// [`SparseBackwardShape`](crate::SparseBackwardShape)-engine flavor: the
/// *shape-generic* fact mechanics plus [`SparseBackwardEffect`]. Any
/// sparse-backward semantics ([`StrongDemand`] today, downstream keys
/// tomorrow) shares this surface: read a converged per-value fact, raise a
/// fact, drain the raised facts into the rule's effect, query the block
/// boundary facts terminator/structured rules map across, and cross callable
/// boundaries. Rules are context-blind: they name bare SSA values and output
/// positions, and the engine qualifies them with the callable context the
/// dispatching work carries.
///
/// Semantics-specific vocabulary lives in helper traits on top — strong
/// liveness's is [`DemandInterp`].
pub trait SparseBackwardInterp:
    Interp<Effect = SparseBackwardEffect<<Self as Interp>::Value>>
{
    /// The converged fact for `value` (bottom if absent).
    fn fact(&self, value: impl Into<SSAValue>) -> Result<Self::Value, Self::Error>;

    /// Merge `fact` into `value`'s fact (buffered; returned by
    /// [`effect`](Self::effect)).
    ///
    /// The engine moves the fact and never inspects it — which element of the
    /// lattice a rule raises is the semantics' business, not the shape's.
    fn raise_fact(
        &mut self,
        value: impl Into<SSAValue>,
        fact: Self::Value,
    ) -> Result<(), Self::Error>;

    /// Drain the raised-fact buffer into this rule's effect.
    fn effect(&mut self) -> Self::Effect;

    /// The parameters of `block` (for mapping parameter facts onto edge or
    /// carried arguments in terminator/structured rules).
    fn block_params(&self, block: Block) -> Result<Vec<SSAValue>, Self::Error>;

    /// The operands of `block`'s terminator — a structured body's yield slots.
    fn terminator_args(&self, block: Block) -> Result<TerminatorArgs, Self::Error>;

    /// Declare the dispatched statement a return site whose operand `i`
    /// carries this callable context's output `i`. Demand requested on an
    /// output merges directly into those operands. Every return site of one
    /// callable must declare the same arity.
    fn declare_return(&mut self, operands: &[SSAValue]) -> Result<(), Self::Error>;

    /// Cross a call boundary backward.
    ///
    /// Resolves `callee` (looked up in `stage`, defaulting to the current
    /// stage) through the engine's linker, selects the callee's context,
    /// merges `outputs[i]` into its output position `i` (activating it on
    /// first use), and returns the facts currently on the callee's
    /// parameters. The dispatching work reruns whenever those parameter facts
    /// rise — including facts that are still bottom now.
    fn summarize_call(
        &mut self,
        callee: Callee,
        stage: Option<CompileStage>,
        outputs: Product<Self::Value>,
    ) -> Result<Product<Self::Value>, Self::Error>;
}

/// [`StrongDemand`]'s helper vocabulary on top of the shape-generic
/// [`SparseBackwardInterp`]: demand is the ⊤ fact, and the purity-aware
/// neededness transfer is the ordinary-dialect one-liner. Strong-liveness
/// rules (`impl Interpretable<I, StrongDemand>`) bound on this trait.
///
/// Pinned to `Semantics = StrongDemand` via the supertrait (rustc elaborates
/// supertraits, so rules bounding `I: DemandInterp` need no extra clauses),
/// and blanket-implemented for every strong-demand sparse-backward engine.
/// [`HasTop`] rides in the same supertrait as an associated-type bound rather
/// than in a `where` clause, for the same reason: elaboration means rules
/// bounding `I: DemandInterp` inherit it and never spell it. (A
/// `where Self::Value: HasTop` on the trait would *not* be elaborated — every
/// rule would have to repeat it.)
pub trait DemandInterp:
    SparseBackwardInterp + Interp<Semantics = StrongDemand, Value: HasTop>
{
    /// Raise `value`'s demand (a demanded value carries the ⊤ fact).
    fn demand(&mut self, value: impl Into<SSAValue>) -> Result<(), Self::Error> {
        self.raise_fact(value, Self::Value::top())
    }

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
    fn demand_uses_if_observable<T>(&mut self, stmt: &T) -> Result<Self::Effect, Self::Error>
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

impl<I> DemandInterp for I
where
    I: SparseBackwardInterp + Interp<Semantics = StrongDemand>,
    I::Value: HasTop,
{
}

// ===========================================================================
// Owners + summaries
// ===========================================================================

/// One discovered callable context: the body it analyzes and what demand
/// learned by running its rules once. Parameters are read from the IR.
pub struct CallableContext {
    pub(super) target: LinkTarget,
    pub(super) body: Body,
    /// `outputs[i]` holds operand `i` of every return site; `None` if the
    /// body declares no return site.
    pub(super) outputs: Option<Vec<Vec<SSAValue>>>,
}

impl CallableContext {
    pub(super) fn new(target: LinkTarget, body: Body) -> Self {
        Self {
            target,
            body,
            outputs: None,
        }
    }

    /// Record a return site: its operand `i` carries output `i`.
    pub(super) fn add_return(&mut self, operands: &[SSAValue]) -> Result<(), InterpreterError> {
        let Some(outputs) = &mut self.outputs else {
            self.outputs = Some(operands.iter().map(|operand| vec![*operand]).collect());
            return Ok(());
        };
        if outputs.len() != operands.len() {
            return Err(InterpreterError::ProductArityMismatch {
                expected: outputs.len(),
                actual: operands.len(),
            });
        }
        for (output, operand) in outputs.iter_mut().zip(operands) {
            output.push(*operand);
        }
        Ok(())
    }

    /// The updates merging a request for `outputs` into the operands of every
    /// return site, after checking its arity. A callable without return sites
    /// accepts any request: no return operand exists that the demand could
    /// reach.
    pub(super) fn route_outputs<K: Clone, V: Clone + PartialEq + HasBottom>(
        &self,
        key: &K,
        outputs: Product<V>,
    ) -> Result<Vec<DemandUpdate<K, V>>, InterpreterError> {
        let Some(operands) = &self.outputs else {
            return Ok(Vec::new());
        };
        if operands.len() != outputs.len() {
            return Err(InterpreterError::ProductArityMismatch {
                expected: operands.len(),
                actual: outputs.len(),
            });
        }
        let mut updates = Vec::new();
        for (operands, fact) in operands.iter().zip(outputs) {
            if fact != V::bottom() {
                for operand in operands {
                    updates.push((
                        Scoped::new(key.clone(), *operand),
                        DemandSummary(fact.clone()),
                    ));
                }
            }
        }
        Ok(updates)
    }
}

/// The driver's `store` slot: the discovered callable contexts. A context is
/// discovered once its entry exists.
pub type DemandContexts<K> = HashMap<K, CallableContext>;

const UNDISCOVERED: InterpreterError =
    InterpreterError::Custom("demand context was never discovered");

/// The per-location summary: one demand fact. Its merge is a real lattice
/// join — `Some(())` iff the fact rose — which is what drives rescheduling.
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

/// The owner-summary type family for the sparse backward engine, over
/// callable-context keys `K`.
pub struct SparseBackwardProfile<V, E, K> {
    #[allow(clippy::type_complexity)]
    _marker: PhantomData<fn() -> (V, E, K)>,
}

impl<'ir, S, V, E, Lk, P, Sem> FixpointProfile<SparseBackwardTransfer<'ir, S, V, E, Lk, P, Sem>>
    for SparseBackwardProfile<V, E, P::Key>
where
    S: StageMeta,
    V: Clone + PartialEq + Lattice,
    E: From<InterpreterError>,
    P: DemandContext<V>,
    Sem: SparseBackwardSemantic,
{
    type SummaryKey = Scoped<P::Key, SSAValue>;
    type Summary = DemandSummary<V>;
    type Frame = DemandFrame<P::Key, V>;
    type Completion = Vec<(SSAValue, V)>;
}

/// One summary update: a context-qualified location and the fact to merge.
pub(super) type DemandUpdate<K, V> = (Scoped<K, SSAValue>, DemandSummary<V>);

/// The sparse backward driver: a [`StandardFixpointInterpreter`] over
/// [`SparseBackwardTransfer`] with context-qualified fact owners and the
/// default self-dependent index (demand propagation).
pub type SparseBackwardDriver<
    'ir,
    S,
    V,
    E,
    Lk = SameStageLinker,
    P = ContextInsensitive,
    Sem = StrongDemand,
> = StandardFixpointInterpreter<
    SparseBackwardTransfer<'ir, S, V, E, Lk, P, Sem>,
    SparseBackwardProfile<V, E, <P as DemandContext<V>>::Key>,
    DemandContexts<<P as DemandContext<V>>::Key>,
    OwnerSummaryDeps<Scoped<<P as DemandContext<V>>::Key, SSAValue>>,
>;

// ===========================================================================
// SparseBackwardTransfer — the summary-free Interp delegate
// ===========================================================================

/// The summary-free transfer of the sparse backward engine: pipeline access,
/// the calling convention and context policy, the real dispatch location and
/// callable context, and the per-rule buffers.
pub struct SparseBackwardTransfer<
    'ir,
    S: StageMeta,
    V,
    E,
    Lk = SameStageLinker,
    P = ContextInsensitive,
    Sem = StrongDemand,
> where
    P: DemandContext<V>,
{
    pipeline: &'ir Pipeline<S>,
    pub(super) linker: Lk,
    pub(super) policy: P,
    location: Option<InterpLocation>,
    /// The callable context of the rule being dispatched, carried by the work
    /// that dispatched it.
    pub(super) context: Option<P::Key>,
    /// Local facts the currently dispatched rule has raised.
    demands: Vec<(SSAValue, V)>,
    /// Callee contexts the current work's call rules crossed into, each with
    /// the facts on the call's results. Applied after the work completes, so
    /// discovery never re-enters rule dispatch.
    pub(super) activations: Vec<(P::Key, LinkTarget, Product<V>)>,
    /// Sparse backward facts live in summaries, not per-call activation
    /// records; this index only satisfies [`Interp::index`].
    activation: EnvIndex,
    _marker: PhantomData<fn() -> (E, Sem)>,
}

impl<'ir, S: StageMeta, V, E, P, Sem> SparseBackwardTransfer<'ir, S, V, E, SameStageLinker, P, Sem>
where
    P: DemandContext<V> + Default,
{
    pub(super) fn new(pipeline: &'ir Pipeline<S>) -> Self {
        Self::with_parts(pipeline, SameStageLinker, P::default())
    }
}

impl<'ir, S: StageMeta, V, E, Lk, P, Sem> SparseBackwardTransfer<'ir, S, V, E, Lk, P, Sem>
where
    P: DemandContext<V>,
{
    pub(super) fn with_parts(pipeline: &'ir Pipeline<S>, linker: Lk, policy: P) -> Self {
        Self {
            pipeline,
            linker,
            policy,
            location: None,
            context: None,
            demands: Vec::new(),
            activations: Vec::new(),
            activation: EnvIndex::placeholder(),
            _marker: PhantomData,
        }
    }

    pub(super) fn into_parts(self) -> (&'ir Pipeline<S>, Lk, P) {
        (self.pipeline, self.linker, self.policy)
    }

    pub fn pipeline(&self) -> &'ir Pipeline<S> {
        self.pipeline
    }

    /// Enter `location` under `context`, returning what to restore.
    pub(super) fn enter(
        &mut self,
        location: InterpLocation,
        context: P::Key,
    ) -> (Option<InterpLocation>, Option<P::Key>) {
        (
            self.location.replace(location),
            self.context.replace(context),
        )
    }

    pub(super) fn restore(&mut self, saved: (Option<InterpLocation>, Option<P::Key>)) {
        (self.location, self.context) = saved;
    }

    pub(super) fn current_context(&self) -> Result<&P::Key, InterpreterError> {
        self.context.as_ref().ok_or(InterpreterError::Custom(
            "no demand context is being dispatched",
        ))
    }

    pub(super) fn activation(&self) -> EnvIndex {
        self.activation
    }
}

impl<'ir, S, V, E, Lk, P, Sem> Interp for SparseBackwardTransfer<'ir, S, V, E, Lk, P, Sem>
where
    S: StageMeta,
    V: Clone,
    E: From<InterpreterError>,
    P: DemandContext<V>,
    Sem: SparseBackwardSemantic,
{
    type Value = V;
    type Error = E;
    type Effect = SparseBackwardEffect<V>;
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

impl<'ir, S, V, E, Lk, P, Sem> AbstractInterpreter
    for SparseBackwardTransfer<'ir, S, V, E, Lk, P, Sem>
where
    S: StageMeta,
    V: Clone,
    E: From<InterpreterError>,
    P: DemandContext<V>,
    Sem: SparseBackwardSemantic,
{
}

// ===========================================================================
// Driver capabilities: the dialect-facing trait
// ===========================================================================

impl<'ir, S, V, E, Lk, P, Sem> SparseBackwardInterp
    for SparseBackwardDriver<'ir, S, V, E, Lk, P, Sem>
where
    S: StageMeta + StageQuery,
    V: Clone + PartialEq + Lattice + HasBottom,
    E: From<InterpreterError>,
    Lk: Linker<S>,
    P: DemandContext<V>,
    Sem: SparseBackwardSemantic,
{
    fn fact(&self, value: impl Into<SSAValue>) -> Result<V, E> {
        let context = self.inner().current_context()?.clone();
        Ok(self.fact_at(Scoped::new(context, value.into())))
    }

    fn raise_fact(&mut self, value: impl Into<SSAValue>, fact: V) -> Result<(), E> {
        let value = value.into();
        self.inner_mut().demands.push((value, fact));
        Ok(())
    }

    fn effect(&mut self) -> SparseBackwardEffect<V> {
        SparseBackwardEffect::Demands(mem::take(&mut self.inner_mut().demands))
    }

    fn block_params(&self, block: Block) -> Result<Vec<SSAValue>, E> {
        query::block_params(self.inner().pipeline(), self.stage(), block).map_err(E::from)
    }

    fn terminator_args(&self, block: Block) -> Result<TerminatorArgs, E> {
        query::terminator_arguments(self.inner().pipeline(), self.stage(), block).map_err(E::from)
    }

    fn declare_return(&mut self, operands: &[SSAValue]) -> Result<(), E> {
        let context = self.inner().current_context()?.clone();
        self.store_mut()
            .get_mut(&context)
            .ok_or(UNDISCOVERED)?
            .add_return(operands)?;
        Ok(())
    }

    fn summarize_call(
        &mut self,
        callee: Callee,
        stage: Option<CompileStage>,
        outputs: Product<V>,
    ) -> Result<Product<V>, E> {
        let lookup_stage = stage.unwrap_or(self.stage());
        let (target, key) =
            self.resolve_call_context(lookup_stage, &callee, &outputs)?;
        let work = self.current_owner().cloned().ok_or_else(|| {
            E::from(InterpreterError::Custom(
                "a call boundary was crossed outside propagation work",
            ))
        })?;

        let params = self.entry_params(target)?;
        let mut facts = Product::new();
        for param in params {
            let location = Scoped::new(key.clone(), param);
            self.dependency_index_mut()
                .register(&location, SummaryDependency::Reanalyze(work.clone()))
                .map_err(|error| E::from(InterpreterError::from(error)))?;
            facts.0.push(self.fact_at(location));
        }
        self.inner_mut().activations.push((key, target, outputs));
        Ok(facts)
    }
}

impl<'ir, S, V, E, Lk, P, Sem> SparseBackwardDriver<'ir, S, V, E, Lk, P, Sem>
where
    S: StageMeta + StageQuery,
    V: Clone + PartialEq + Lattice + HasBottom,
    E: From<InterpreterError>,
    Lk: Linker<S>,
    P: DemandContext<V>,
    Sem: SparseBackwardSemantic,
{
    /// Resolve a callable and select its demand context.
    fn resolve_call_context(
        &mut self,
        lookup_stage: CompileStage,
        callee: &Callee,
        outputs: &Product<V>,
    ) -> Result<(LinkTarget, P::Key), E> {
        let target = self.inner().linker.resolve(
            self.inner().pipeline(),
            lookup_stage,
            callee,
        )?;
        let key = self.inner_mut().policy.key(&target, outputs);
        Ok((target, key))
    }

    /// The current fact at a context-qualified location (bottom if absent).
    pub(super) fn fact_at(&self, location: Scoped<P::Key, SSAValue>) -> V {
        self.summary(&location)
            .map(|summary| summary.0.clone())
            .unwrap_or_else(V::bottom)
    }

    /// `target`'s callable body and entry block. Unsupported bodies fail here
    /// rather than producing empty facts.
    fn callable_entry(&self, target: LinkTarget) -> Result<(Body, Block), E> {
        let pipeline = self.inner().pipeline();
        let body = target.body(pipeline)?;
        let entry = match body {
            Body::CFG(cfg) => {
                query::cfg_entry(pipeline, target.stage, cfg)?.ok_or(InterpreterError::EmptyCFG)?
            }
            Body::Block(block) => block,
            Body::DiGraph(_) | Body::UnGraph(_) => {
                return Err(E::from(InterpreterError::NoDefaultWalker(body)));
            }
        };
        Ok((body, entry))
    }

    /// `target`'s entry parameters — what callers map onto arguments.
    pub(super) fn entry_params(&self, target: LinkTarget) -> Result<Vec<SSAValue>, E> {
        let (_, entry) = self.callable_entry(target)?;
        Ok(query::block_params(
            self.inner().pipeline(),
            target.stage,
            entry,
        )?)
    }
}

impl<'ir, S, V, E, Lk, P, Sem> SparseBackwardDriver<'ir, S, V, E, Lk, P, Sem>
where
    S: StageMeta + StageQuery + InterpDispatch<Self>,
    V: Clone + PartialEq + Lattice + HasBottom,
    E: From<InterpreterError>,
    Lk: Linker<S>,
    P: DemandContext<V>,
    Sem: SparseBackwardSemantic,
{
    /// Dispatch one statement's backward rule at `stage` under `context`.
    pub(super) fn run_statement(
        &mut self,
        context: &P::Key,
        stage: CompileStage,
        statement: Statement,
    ) -> Result<SparseBackwardEffect<V>, E> {
        let pipeline = self.inner().pipeline();
        let info = pipeline
            .stage(stage)
            .ok_or_else(|| E::from(InterpreterError::MissingStage(stage)))?;
        let index = self.inner().activation();
        let saved = self.inner_mut().enter(
            InterpLocation {
                stage,
                statement,
                index,
            },
            context.clone(),
        );
        let result = info.dispatch_statement(statement, self);
        self.inner_mut().restore(saved);
        result
    }

    /// Discover a context once: record it, then evaluate every rule of
    /// its body and structured children — but not nested callable
    /// definitions — with nothing demanded yet, collecting the unconditional
    /// contributions (control roots) before any of them is merged. Rules
    /// whose contributions are conditional read bottom here and rerun when
    /// the facts they consulted rise.
    pub(super) fn discover(
        &mut self,
        key: &P::Key,
        target: LinkTarget,
    ) -> Result<Vec<DemandUpdate<P::Key, V>>, E> {
        if let Some(context) = self.store().get(key) {
            if context.target != target {
                return Err(E::from(InterpreterError::Custom(
                    "demand context key selected for two call targets",
                )));
            }
            return Ok(Vec::new());
        }
        let (body, _) = self.callable_entry(target)?;
        self.store_mut()
            .insert(key.clone(), CallableContext::new(target, body));
        let stage = target.stage;

        let mut bodies = VecDeque::from([body]);
        let mut visited = HashSet::new();
        let mut seeds = Vec::new();
        while let Some(body) = bodies.pop_front() {
            if !visited.insert(body) {
                continue;
            }
            let contents = query::body_contents(self.inner().pipeline(), stage, body)?;
            bodies.extend(contents.children);
            for statement in contents.statements {
                let SparseBackwardEffect::Demands(demands) =
                    self.run_statement(key, stage, statement)?;
                seeds.extend(
                    demands.into_iter().map(|(value, fact)| {
                        (Scoped::new(key.clone(), value), DemandSummary(fact))
                    }),
                );
            }
        }
        Ok(seeds)
    }

    /// Apply the call boundaries crossed by the work that just completed:
    /// discover each callee context if needed, then route the call's result
    /// facts into its return operands. Every update still merges through the
    /// driver.
    pub(super) fn apply_activations(&mut self) -> Result<Vec<DemandUpdate<P::Key, V>>, E> {
        let mut updates = Vec::new();
        loop {
            let activations = mem::take(&mut self.inner_mut().activations);
            if activations.is_empty() {
                return Ok(updates);
            }
            for (key, target, outputs) in activations {
                updates.extend(self.discover(&key, target)?);
                updates.extend(
                    self.store()
                        .get(&key)
                        .ok_or(UNDISCOVERED)?
                        .route_outputs(&key, outputs)?,
                );
            }
        }
    }
}

// ===========================================================================
// Owner semantics: analyzing one location = running its translating rules
// ===========================================================================

pub(super) struct SparseBackwardOwnerAnalysis;

impl<'ir, S, V, E, Lk, P, Sem>
    OwnerAnalysis<
        SparseBackwardDriver<'ir, S, V, E, Lk, P, Sem>,
        Scoped<P::Key, SSAValue>,
        DemandSummary<V>,
        DemandFrame<P::Key, V>,
        Vec<(SSAValue, V)>,
        E,
    > for SparseBackwardOwnerAnalysis
where
    S: StageMeta + StageQuery + InterpDispatch<SparseBackwardDriver<'ir, S, V, E, Lk, P, Sem>>,
    V: Clone + PartialEq + Lattice + HasBottom,
    E: From<InterpreterError>,
    Lk: Linker<S>,
    P: DemandContext<V>,
    Sem: SparseBackwardSemantic,
{
    fn bottom_summary(
        &mut self,
        _interp: &mut SparseBackwardDriver<'ir, S, V, E, Lk, P, Sem>,
        _owner: &Scoped<P::Key, SSAValue>,
    ) -> Result<DemandSummary<V>, E> {
        Ok(DemandSummary(V::bottom()))
    }

    fn entry_frame(
        &mut self,
        interp: &mut SparseBackwardDriver<'ir, S, V, E, Lk, P, Sem>,
        owner: &Scoped<P::Key, SSAValue>,
        _summary: &DemandSummary<V>,
    ) -> Result<DemandFrame<P::Key, V>, E> {
        let context = interp.store().get(&owner.scope).ok_or(UNDISCOVERED)?;
        let stage = context.target.stage;
        let pipeline = interp.inner().pipeline();
        let work = match query::value_kind(pipeline, stage, owner.item)? {
            SSAKind::Result(statement, _) => vec![statement],
            SSAKind::BlockArgument(block, _) => {
                query::block_argument_predecessors(pipeline, stage, block)?.into_vec()
            }
            SSAKind::Port(parent, _) => {
                vec![query::graph_port_owner(pipeline, stage, parent)?]
            }
        };
        Ok(DemandFrame::new(owner.scope.clone(), stage, work))
    }

    fn complete_owner(
        &mut self,
        interp: &mut SparseBackwardDriver<'ir, S, V, E, Lk, P, Sem>,
        owner: Scoped<P::Key, SSAValue>,
        completion: Vec<(SSAValue, V)>,
    ) -> Result<SummaryEffect<Scoped<P::Key, SSAValue>, DemandSummary<V>>, E> {
        // Qualify the rules' local demands with the work's context, then
        // apply the call boundaries the work crossed.
        let mut updates: Vec<DemandUpdate<P::Key, V>> = completion
            .into_iter()
            .map(|(value, fact)| (Scoped::new(owner.scope.clone(), value), DemandSummary(fact)))
            .collect();
        updates.extend(interp.apply_activations()?);
        Ok(SummaryEffect::Many(updates))
    }
}

// ===========================================================================
// The public engine
// ===========================================================================

/// Sparse backward demand engine (strong liveness / neededness).
///
/// Demand crosses supported calls: a demanded call result demands the
/// matching output of the callee's context, and demanded callee parameters
/// demand the matching call arguments. Facts are qualified by the callable
/// context the [`DemandContext`] policy `P` selects (default: one context per
/// resolved [`LinkTarget`](crate::LinkTarget)).
///
/// `analyze` consumes the engine and hands it back only on success, so a
/// failed analysis can never be resumed: build a new engine instead.
/// Successful analyses accumulate: discovered contexts and their facts are
/// reused, so shared callees are analyzed once. Facts only rise, so a request
/// whose policy key matches an earlier context returns the join with the
/// earlier requests — never less demanded than a fresh engine, possibly more,
/// and dependent on query order. That happens for every repeated callee under
/// [`ContextInsensitive`], and under a bounded policy once its fallback
/// context is shared. Use a fresh engine for an isolated answer.
///
/// ```ignore
/// let analysis = SparseBackwardInterpreter::<Stage, Demand>::new(&pipeline);
/// let (params, analysis) = analysis.analyze(stage, callee, [Demand::Live])?;
/// let demanded = analysis.is_demanded(analysis.root().unwrap(), value);
/// ```
pub struct SparseBackwardInterpreter<
    'ir,
    S: StageMeta,
    V,
    E = InterpreterError,
    Lk = SameStageLinker,
    P = ContextInsensitive,
    Sem = StrongDemand,
> where
    V: Clone + PartialEq + Lattice,
    E: From<InterpreterError>,
    P: DemandContext<V>,
    Sem: SparseBackwardSemantic,
{
    pub(super) driver: SparseBackwardDriver<'ir, S, V, E, Lk, P, Sem>,
    pub(super) root: Option<P::Key>,
}

impl<'ir, S, V, E, P, Sem> SparseBackwardInterpreter<'ir, S, V, E, SameStageLinker, P, Sem>
where
    S: StageMeta,
    V: Clone + PartialEq + Lattice,
    E: From<InterpreterError>,
    P: DemandContext<V> + Default,
    Sem: SparseBackwardSemantic,
{
    pub fn new(pipeline: &'ir Pipeline<S>) -> Self {
        Self::from_transfer(SparseBackwardTransfer::new(pipeline))
    }
}

impl<'ir, S, V, E, Lk, P, Sem> SparseBackwardInterpreter<'ir, S, V, E, Lk, P, Sem>
where
    S: StageMeta,
    V: Clone + PartialEq + Lattice,
    E: From<InterpreterError>,
    P: DemandContext<V>,
    Sem: SparseBackwardSemantic,
{
    fn from_transfer(transfer: SparseBackwardTransfer<'ir, S, V, E, Lk, P, Sem>) -> Self {
        Self {
            root: None,
            driver: StandardFixpointInterpreter::with_dependency_index(
                transfer,
                Default::default(),
                (),
                OwnerSummaryDeps::new(),
            ),
        }
    }

    /// Replace the calling convention used for roots and calls. Resets the
    /// analysis state.
    pub fn with_linker<Lk2>(
        self,
        linker: Lk2,
    ) -> SparseBackwardInterpreter<'ir, S, V, E, Lk2, P, Sem> {
        let (pipeline, _, policy) = self.driver.into_inner().into_parts();
        SparseBackwardInterpreter::from_transfer(SparseBackwardTransfer::with_parts(
            pipeline, linker, policy,
        ))
    }

    /// Replace the context-selection policy (and so the context key type).
    /// Resets the analysis state.
    pub fn with_policy<P2: DemandContext<V>>(
        self,
        policy: P2,
    ) -> SparseBackwardInterpreter<'ir, S, V, E, Lk, P2, Sem> {
        let (pipeline, linker, _) = self.driver.into_inner().into_parts();
        SparseBackwardInterpreter::from_transfer(SparseBackwardTransfer::with_parts(
            pipeline, linker, policy,
        ))
    }

    pub fn pipeline(&self) -> &'ir Pipeline<S> {
        self.driver.inner().pipeline()
    }

    /// The root of the most recent analysis; detailed facts remain available
    /// through context queries.
    pub fn root(&self) -> Option<&P::Key> {
        self.root.as_ref()
    }

    /// The discovered callable contexts, in no particular order.
    pub fn contexts(&self) -> impl Iterator<Item = &P::Key> {
        self.driver.store().keys()
    }

    /// The `(target stage, callable body)` a discovered context analyzes.
    pub fn scope(&self, context: &P::Key) -> Option<BodyScope> {
        self.driver
            .store()
            .get(context)
            .map(|context| (context.target.stage, context.body))
    }

    /// The converged demand fact for `value` in `context`.
    pub fn fact(&self, context: &P::Key, value: impl Into<SSAValue>) -> Option<&V> {
        self.driver
            .summary(&Scoped::new(context.clone(), value.into()))
            .map(|summary| &summary.0)
    }

    /// The joined demand on operand `index` of every return site of `context`.
    pub fn result_value(&self, context: &P::Key, index: usize) -> Option<V> {
        let store = self.driver.store();
        let operands = store.get(context)?.outputs.as_ref()?.get(index)?;
        operands
            .iter()
            .filter_map(|operand| self.fact(context, *operand))
            .cloned()
            .reduce(|joined, fact| joined.join(&fact))
    }

    /// All converged `(value, fact)` pairs in `context`.
    pub fn facts<'a>(&'a self, context: &'a P::Key) -> impl Iterator<Item = (SSAValue, &'a V)> {
        self.driver
            .summaries()
            .iter()
            .filter(move |(owner, _)| owner.scope == *context)
            .map(|(owner, summary)| (owner.item, &summary.0))
    }

    /// The converged value facts in `context` as a [`SparseStore`] (the
    /// sparse per-SSA-value fact view; absent = bottom).
    pub fn fact_store(&self, context: &P::Key) -> SparseStore<V> {
        let mut store = SparseStore::new();
        for (value, fact) in self.facts(context) {
            store.set(value, fact.clone());
        }
        store
    }

    /// `true` iff `value` carries a non-bottom demand fact in `context`.
    pub fn is_demanded(&self, context: &P::Key, value: impl Into<SSAValue>) -> bool
    where
        V: HasBottom,
    {
        self.fact(context, value)
            .is_some_and(|fact| *fact != V::bottom())
    }
}

impl<'ir, S, V, E, Lk, P, Sem> SparseBackwardInterpreter<'ir, S, V, E, Lk, P, Sem>
where
    S: StageMeta + StageQuery + InterpDispatch<SparseBackwardDriver<'ir, S, V, E, Lk, P, Sem>>,
    V: Clone + PartialEq + Lattice + HasBottom,
    E: From<InterpreterError>,
    Lk: Linker<S>,
    P: DemandContext<V>,
    Sem: SparseBackwardSemantic,
{
    /// Resolve a stage and function by name and infer parameter facts from
    /// the supplied output facts. See [`Self::analyze`].
    pub fn analyze_by_name(
        self,
        stage_name: &str,
        function_name: &str,
        result_values: impl IntoIterator<Item = V>,
    ) -> Result<(Product<V>, Self), E> {
        let stage = self
            .pipeline()
            .stage_by_name(stage_name)
            .ok_or_else(|| E::from(InterpreterError::MissingStageName(stage_name.into())))?;
        let function = self
            .pipeline()
            .lookup_function_by_name(function_name)
            .ok_or_else(|| E::from(InterpreterError::MissingFunctionName(function_name.into())))?;
        self.analyze(stage, Callee::Function(function), result_values)
    }

    /// Resolve a stage-local symbol and infer parameter facts from the
    /// supplied output facts. See [`Self::analyze`].
    pub fn analyze_by_symbol(
        self,
        stage: CompileStage,
        symbol: Symbol,
        result_values: impl IntoIterator<Item = V>,
    ) -> Result<(Product<V>, Self), E> {
        self.analyze(stage, symbol.into(), result_values)
    }

    /// Infer one fact per entry parameter, in parameter order, from output
    /// lattice values. Supply exactly one lattice value per result.
    ///
    /// Discover return sites and intrinsic roots once per context, merge the
    /// supplied output facts into the return operands, then drain the shared
    /// fixpoint worklist. Bottom output facts still allow intrinsic roots to
    /// contribute parameter facts.
    /// Facts join with prior requests if the context policy reuses a key.
    /// Returns the engine on success, so detailed context facts remain
    /// accessible through [`Self::root`], [`Self::contexts`], and
    /// [`Self::fact`]; an error drops it.
    pub fn analyze(
        mut self,
        stage: CompileStage,
        callee: Callee,
        result_values: impl IntoIterator<Item = V>,
    ) -> Result<(Product<V>, Self), E> {
        let outputs: Product<V> = result_values.into_iter().collect();
        let (target, root) =
            self.driver.resolve_call_context(stage, &callee, &outputs)?;
        self.driver
            .inner_mut()
            .activations
            .push((root.clone(), target, outputs));
        let updates = self.driver.apply_activations()?;
        let mut owner_analysis = SparseBackwardOwnerAnalysis;
        for (location, fact) in updates {
            self.driver
                .merge_summary(&mut owner_analysis, location, fact)?;
        }
        self.driver.drain_worklist(&mut owner_analysis)?;
        let params = self
            .driver
            .entry_params(target)?
            .into_iter()
            .map(|param| self.driver.fact_at(Scoped::new(root.clone(), param)))
            .collect();
        self.root = Some(root);
        Ok((params, self))
    }
}
