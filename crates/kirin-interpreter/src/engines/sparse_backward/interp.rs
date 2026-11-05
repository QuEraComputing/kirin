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
//! # Layering
//!
//! [`SparseBackwardInterpreter`] is the public engine. Internally it is a
//! wrapper over a [`StandardFixpointInterpreter`] driving a
//! [`SparseBackwardTransfer`]:
//!
//! - **[`SparseBackwardTransfer`]** is the [`Interp`] delegate and the target
//!   of rule dispatch: pipeline access, the calling convention and context
//!   policy, the demand facts (one [`EnvStore`] environment per selected
//!   context), the discovered contexts' metadata, the dispatch location, the
//!   facts that changed, and the pending call activations.
//! - the **[`StandardFixpointInterpreter`]** driver owns the worklist and the
//!   call-boundary callers ([`DemandCallers`]); after each work item it drains
//!   the changed facts and reruns their dependents.
//!
//! # Work is a statement; a fact is a value
//!
//! A work item is `(EnvIndex, Statement)`: rerun one statement's rule in one
//! context. A fact is keyed `(EnvIndex, SSAValue)`. When a value's demand
//! rises (an [`EnvStore::join`] that reports a change), the driver reruns the
//! statements that translate it, which SSA topology determines:
//!
//! - a statement **result** → the defining statement's backward rule;
//! - a **block argument** → its directly owning structured statement, or each
//!   indexed CFG predecessor block's terminator;
//! - a graph **port** → the statement owning the graph boundary.
//!
//! Requested outputs merge directly into the operands of the context's return
//! sites (declared through [`declare_return`](SparseBackwardInterp::declare_return)).
//!
//! Call rules return [`SparseBackwardEffect::Call`]; the [`DemandFrame`]
//! summarizes the call: it selects the callee's context (allocating its
//! environment at once), queues its discovery, and registers the one
//! dependency topology does not imply: callee parameter facts → the caller's
//! call statement. Discovery itself stays deferred until the work completes.
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
    AbstractInterpreter, Body, CallEffect, Callee, ContextInsensitive, EnvIndex, EnvStore,
    FixpointProfile, Interp, InterpDispatch, InterpLocation, InterpreterError, LinkTarget, Linker,
    SameStageLinker, SparseBackwardSemantic, SparseStore, StageQuery, StandardFixpointInterpreter,
    StrongDemand, TerminatorArgs, WorkAnalysis,
};

/// A resolved `(stage, body)` pair: the structure one callable context
/// analyzes.
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

/// What a backward rule produced.
///
/// Rules raise facts imperatively through
/// [`raise_fact`](SparseBackwardInterp::raise_fact) (or its [`DemandInterp`]
/// spelling [`demand`](DemandInterp::demand)), which joins into the rule's
/// callable context at once, and finish with [`Next`](Self::Next). A call
/// rule returns [`Call`](Self::Call) instead.
pub enum SparseBackwardEffect<V> {
    /// The rule is done.
    Next,
    /// Cross a call: `inputs` are the facts on the call's results, `outputs`
    /// the caller's argument slots. The engine raises the callee's current
    /// parameter facts on those arguments.
    Call(CallEffect<V>),
}

/// [`SparseBackwardShape`](crate::SparseBackwardShape)-engine flavor: the
/// *shape-generic* fact mechanics plus [`SparseBackwardEffect`]. Any
/// sparse-backward semantics ([`StrongDemand`] today, downstream keys
/// tomorrow) shares this surface: read a converged per-value fact, raise a
/// fact, query the block
/// boundary facts terminator/structured rules map across, and declare a
/// callable's return sites. Rules are context-blind: they name bare SSA values and output
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

    /// Join `fact` into `value`'s fact in the dispatched context.
    ///
    /// The engine moves the fact and never inspects it — which element of the
    /// lattice a rule raises is the semantics' business, not the shape's.
    fn raise_fact(
        &mut self,
        value: impl Into<SSAValue>,
        fact: Self::Value,
    ) -> Result<(), Self::Error>;

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
        Ok(SparseBackwardEffect::Next)
    }
}

impl<I> DemandInterp for I
where
    I: SparseBackwardInterp + Interp<Semantics = StrongDemand>,
    I::Value: HasTop,
{
}

// ===========================================================================
// Callable contexts, work items, and the driver
// ===========================================================================

/// One discovered callable context: the body it analyzes and what demand
/// learned by running its rules once. Parameters are read from the IR. This
/// is metadata, not facts: the facts live in the context's environment.
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

    /// The facts merging a request for `outputs` into the operands of every
    /// return site, after checking its arity. A callable without return sites
    /// accepts any request: no return operand exists that the demand could
    /// reach.
    pub(super) fn route_outputs<V: Clone>(
        &self,
        outputs: Product<V>,
    ) -> Result<Vec<(SSAValue, V)>, InterpreterError> {
        let Some(operands) = &self.outputs else {
            return Ok(Vec::new());
        };
        if operands.len() != outputs.len() {
            return Err(InterpreterError::ProductArityMismatch {
                expected: operands.len(),
                actual: outputs.len(),
            });
        }
        Ok(operands
            .iter()
            .zip(outputs)
            .flat_map(|(operands, fact)| {
                operands.iter().map(move |operand| (*operand, fact.clone()))
            })
            .collect())
    }
}

const UNDISCOVERED: InterpreterError =
    InterpreterError::Custom("demand context was never discovered");

const CALL_OUTSIDE_WORK: InterpreterError =
    InterpreterError::Custom("a call boundary was crossed outside propagation work");

/// One demand work item: rerun `Statement`'s backward rule in the context
/// whose environment is `EnvIndex`.
pub(super) type DemandWork = (EnvIndex, Statement);

/// The driver's `store` slot: the callers to rerun when a callee context's
/// parameter fact rises. This is the one dependency SSA topology does not
/// imply.
pub type DemandCallers = HashMap<(EnvIndex, SSAValue), Vec<DemandWork>>;

/// The work-item type family for the sparse backward engine. Facts live in
/// the transfer's [`EnvStore`].
pub struct SparseBackwardProfile<V, E> {
    _marker: PhantomData<fn() -> (V, E)>,
}

impl<'ir, S, V, E, Lk, P, Sem> FixpointProfile<SparseBackwardTransfer<'ir, S, V, E, Lk, P, Sem>>
    for SparseBackwardProfile<V, E>
where
    S: StageMeta,
    V: Clone + PartialEq + Lattice,
    E: From<InterpreterError>,
    P: DemandContext<V>,
    Sem: SparseBackwardSemantic,
{
    type Work = DemandWork;
    type Frame = DemandFrame;
    type Completion = ();
}

/// The sparse backward driver: a [`StandardFixpointInterpreter`] over
/// [`SparseBackwardTransfer`] whose work items are statements in a context,
/// with the call-boundary callers in its `store` slot.
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
    SparseBackwardProfile<V, E>,
    DemandCallers,
>;

// ===========================================================================
// SparseBackwardTransfer — facts, metadata, and rule dispatch
// ===========================================================================

/// The transfer of the sparse backward engine and the target of rule
/// dispatch: pipeline access, the calling convention and context policy, the
/// demand facts of every context, the discovered contexts' metadata, the
/// dispatch location, and the per-rule buffers.
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
    /// Demand facts, one environment per selected context.
    pub(super) env: EnvStore<P::Key, SSAValue, V>,
    /// Discovered contexts, by environment.
    pub(super) contexts: HashMap<EnvIndex, CallableContext>,
    /// The environment of the rule being dispatched.
    context: Option<EnvIndex>,
    /// Facts that rose since the driver last reran their dependents.
    changed: Vec<(EnvIndex, SSAValue)>,
    /// Callee contexts the current work's call rules crossed into, each with
    /// the facts on the call's results. Applied after the work completes, so
    /// discovery never re-enters rule dispatch.
    activations: Vec<(EnvIndex, LinkTarget, Product<V>)>,
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
            env: EnvStore::new(),
            contexts: HashMap::new(),
            context: None,
            changed: Vec::new(),
            activations: Vec::new(),
            _marker: PhantomData,
        }
    }

    pub(super) fn into_parts(self) -> (&'ir Pipeline<S>, Lk, P) {
        (self.pipeline, self.linker, self.policy)
    }

    pub fn pipeline(&self) -> &'ir Pipeline<S> {
        self.pipeline
    }

    /// Join `fact` into `value`'s demand in `env`, recording it if it rose.
    pub(super) fn raise(
        &mut self,
        env: EnvIndex,
        value: SSAValue,
        fact: &V,
    ) -> Result<(), InterpreterError>
    where
        V: Lattice + HasBottom + Clone,
    {
        if self.env.join(env, value, fact)? {
            self.changed.push((env, value));
        }
        Ok(())
    }

    fn current_context(&self) -> Result<EnvIndex, InterpreterError> {
        self.context.ok_or(InterpreterError::Custom(
            "no demand context is being dispatched",
        ))
    }

    /// The environment of a discovered context.
    pub(super) fn discovered(&self, key: &P::Key) -> Option<EnvIndex> {
        self.env
            .context_env(key)
            .filter(|env| self.contexts.contains_key(env))
    }
}

impl<'ir, S, V, E, Lk, P, Sem> SparseBackwardTransfer<'ir, S, V, E, Lk, P, Sem>
where
    S: StageMeta + StageQuery + InterpDispatch<Self>,
    V: Clone + PartialEq + Lattice + HasBottom,
    E: From<InterpreterError>,
    Lk: Linker<S>,
    P: DemandContext<V>,
    Sem: SparseBackwardSemantic,
{
    /// Resolve a callable, select its demand context, allocate the context's
    /// environment (so callers can subscribe to its parameters), and queue
    /// its activation. Discovery stays deferred until the current work
    /// completes.
    pub(super) fn submit_call(
        &mut self,
        lookup_stage: CompileStage,
        callee: &Callee,
        outputs: Product<V>,
    ) -> Result<(LinkTarget, P::Key, EnvIndex), E> {
        let target = self.linker.resolve(self.pipeline, lookup_stage, callee)?;
        let key = self.policy.key(&target, &outputs);
        let env = self.env.get_or_allocate(key.clone());
        self.activations.push((env, target, outputs));
        Ok((target, key, env))
    }

    /// Dispatch one statement's backward rule at `stage` in context `env`
    /// and return its effect. The caller decides what a
    /// [`Call`](SparseBackwardEffect::Call) means: a [`DemandFrame`]
    /// summarizes it, discovery rejects it.
    pub(super) fn run_statement(
        &mut self,
        env: EnvIndex,
        stage: CompileStage,
        statement: Statement,
    ) -> Result<SparseBackwardEffect<V>, E> {
        let info = self
            .pipeline
            .stage(stage)
            .ok_or_else(|| E::from(InterpreterError::MissingStage(stage)))?;
        let location = InterpLocation {
            stage,
            statement,
            index: env,
        };
        let saved = (self.location.replace(location), self.context.replace(env));
        let result = info.dispatch_statement(statement, self);
        (self.location, self.context) = saved;
        result
    }

    /// Discover a context once: record it, then evaluate every rule of
    /// its body and structured children — but not nested callable
    /// definitions — once, raising the unconditional contributions (control
    /// roots). Rules whose contributions are conditional rerun when the facts
    /// they consulted rise. Discovery runs outside any work, so it rejects a
    /// call; the shipped call rule never crosses here before its results are
    /// demanded.
    fn discover(&mut self, env: EnvIndex, target: LinkTarget) -> Result<(), E> {
        if let Some(context) = self.contexts.get(&env) {
            if context.target != target {
                return Err(E::from(InterpreterError::Custom(
                    "demand context key selected for two call targets",
                )));
            }
            return Ok(());
        }
        let (body, _) = self.callable_entry(target)?;
        self.contexts
            .insert(env, CallableContext::new(target, body));
        let stage = target.stage;

        let mut bodies = VecDeque::from([body]);
        let mut visited = HashSet::new();
        while let Some(body) = bodies.pop_front() {
            if !visited.insert(body) {
                continue;
            }
            let contents = query::body_contents(self.pipeline, stage, body)?;
            bodies.extend(contents.children);
            for statement in contents.statements {
                if let SparseBackwardEffect::Call(_) = self.run_statement(env, stage, statement)? {
                    return Err(E::from(CALL_OUTSIDE_WORK));
                }
            }
        }
        Ok(())
    }

    /// Apply the call boundaries crossed by the work that just completed:
    /// discover each callee context if needed, then raise the call's result
    /// facts on its return operands.
    pub(super) fn apply_activations(&mut self) -> Result<(), E> {
        loop {
            let activations = mem::take(&mut self.activations);
            if activations.is_empty() {
                return Ok(());
            }
            for (env, target, outputs) in activations {
                self.discover(env, target)?;
                let routed = self
                    .contexts
                    .get(&env)
                    .ok_or(UNDISCOVERED)?
                    .route_outputs(outputs)?;
                for (operand, fact) in routed {
                    self.raise(env, operand, &fact)?;
                }
            }
        }
    }

    /// The statements that translate `value`'s demand in `env`, as work:
    /// a statement result → its defining statement; a block argument → its
    /// owning structured statement or each indexed CFG predecessor's
    /// terminator; a graph port → the statement owning the graph boundary.
    pub(super) fn dependent_work(
        &self,
        env: EnvIndex,
        value: SSAValue,
    ) -> Result<Vec<DemandWork>, E> {
        let stage = self.contexts.get(&env).ok_or(UNDISCOVERED)?.target.stage;
        let statements = match query::value_kind(self.pipeline, stage, value)? {
            SSAKind::Result(statement, _) => vec![statement],
            SSAKind::BlockArgument(block, _) => {
                query::block_argument_predecessors(self.pipeline, stage, block)?.into_vec()
            }
            SSAKind::Port(parent, _) => {
                vec![query::graph_port_owner(self.pipeline, stage, parent)?]
            }
        };
        Ok(statements
            .into_iter()
            .map(|statement| (env, statement))
            .collect())
    }

    /// The current facts on a context's entry parameters.
    pub(super) fn param_facts(&self, env: EnvIndex, params: &[SSAValue]) -> Result<Product<V>, E> {
        params
            .iter()
            .map(|param| Ok::<V, E>(self.env.read(env, *param)?.unwrap_or_else(V::bottom)))
            .collect()
    }

    /// `target`'s callable body and entry block. Unsupported bodies fail here
    /// rather than producing empty facts.
    fn callable_entry(&self, target: LinkTarget) -> Result<(Body, Block), E> {
        let body = target.body(self.pipeline)?;
        let entry = match body {
            Body::CFG(cfg) => query::cfg_entry(self.pipeline, target.stage, cfg)?
                .ok_or(InterpreterError::EmptyCFG)?,
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
        Ok(query::block_params(self.pipeline, target.stage, entry)?)
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
        self.location.expect("interp location not set").index
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

impl<'ir, S, V, E, Lk, P, Sem> SparseBackwardInterp
    for SparseBackwardTransfer<'ir, S, V, E, Lk, P, Sem>
where
    S: StageMeta + StageQuery,
    V: Clone + PartialEq + Lattice + HasBottom,
    E: From<InterpreterError>,
    P: DemandContext<V>,
    Sem: SparseBackwardSemantic,
{
    fn fact(&self, value: impl Into<SSAValue>) -> Result<V, E> {
        let env = self.current_context()?;
        Ok(self.env.read(env, value.into())?.unwrap_or_else(V::bottom))
    }

    fn raise_fact(&mut self, value: impl Into<SSAValue>, fact: V) -> Result<(), E> {
        let env = self.current_context()?;
        Ok(self.raise(env, value.into(), &fact)?)
    }

    fn block_params(&self, block: Block) -> Result<Vec<SSAValue>, E> {
        query::block_params(self.pipeline, self.stage(), block).map_err(E::from)
    }

    fn terminator_args(&self, block: Block) -> Result<TerminatorArgs, E> {
        query::terminator_arguments(self.pipeline, self.stage(), block).map_err(E::from)
    }

    fn declare_return(&mut self, operands: &[SSAValue]) -> Result<(), E> {
        let env = self.current_context()?;
        self.contexts
            .get_mut(&env)
            .ok_or(UNDISCOVERED)?
            .add_return(operands)?;
        Ok(())
    }
}

// ===========================================================================
// Driver: call-boundary dependencies and scheduling
// ===========================================================================

impl<'ir, S, V, E, Lk, P, Sem> SparseBackwardDriver<'ir, S, V, E, Lk, P, Sem>
where
    S: StageMeta + StageQuery + InterpDispatch<SparseBackwardTransfer<'ir, S, V, E, Lk, P, Sem>>,
    V: Clone + PartialEq + Lattice + HasBottom,
    E: From<InterpreterError>,
    Lk: Linker<S>,
    P: DemandContext<V>,
    Sem: SparseBackwardSemantic,
{
    /// Summarize a call without descending: queue the callee context,
    /// register the caller's work on each callee parameter fact, and raise
    /// the callee's current parameter facts on the call's arguments.
    pub(super) fn summarize_call(
        &mut self,
        caller_stage: CompileStage,
        call: CallEffect<V>,
    ) -> Result<(), E> {
        let (target, _, callee) = self.inner_mut().submit_call(
            call.stage.unwrap_or(caller_stage),
            &call.callee,
            call.inputs,
        )?;
        let work = *self
            .current_work()
            .ok_or_else(|| E::from(CALL_OUTSIDE_WORK))?;
        let params = self.inner().entry_params(target)?;
        if params.len() != call.outputs.len() {
            return Err(E::from(InterpreterError::ProductArityMismatch {
                expected: params.len(),
                actual: call.outputs.len(),
            }));
        }
        for param in &params {
            let callers = self.store_mut().entry((callee, *param)).or_default();
            if !callers.contains(&work) {
                callers.push(work);
            }
        }
        let facts = self.inner().param_facts(callee, &params)?;
        for (argument, fact) in call.outputs.into_iter().zip(facts) {
            self.inner_mut().raise(work.0, argument, &fact)?;
        }
        Ok(())
    }

    /// Rerun the dependents of every fact that changed: the statements that
    /// translate it and the callers registered on it.
    pub(super) fn wake_changed(&mut self) -> Result<(), E> {
        for (env, value) in mem::take(&mut self.inner_mut().changed) {
            for work in self.inner().dependent_work(env, value)? {
                self.schedule_work(work);
            }
            let callers = self.store().get(&(env, value)).cloned().unwrap_or_default();
            for work in callers {
                self.schedule_work(work);
            }
        }
        Ok(())
    }

    fn schedule_work(&mut self, work: DemandWork) {
        self.schedule(work);
    }
}

// ===========================================================================
// Work analysis: one work item = one statement's rule in one context
// ===========================================================================

pub(super) struct SparseBackwardWorkAnalysis;

impl<'ir, S, V, E, Lk, P, Sem>
    WorkAnalysis<SparseBackwardDriver<'ir, S, V, E, Lk, P, Sem>, DemandWork, DemandFrame, (), E>
    for SparseBackwardWorkAnalysis
where
    S: StageMeta + StageQuery + InterpDispatch<SparseBackwardTransfer<'ir, S, V, E, Lk, P, Sem>>,
    V: Clone + PartialEq + Lattice + HasBottom,
    E: From<InterpreterError>,
    Lk: Linker<S>,
    P: DemandContext<V>,
    Sem: SparseBackwardSemantic,
{
    fn entry_frame(
        &mut self,
        interp: &mut SparseBackwardDriver<'ir, S, V, E, Lk, P, Sem>,
        &(env, statement): &DemandWork,
    ) -> Result<DemandFrame, E> {
        let stage = interp
            .inner()
            .contexts
            .get(&env)
            .ok_or(UNDISCOVERED)?
            .target
            .stage;
        Ok(DemandFrame::new(env, stage, statement))
    }

    fn complete_work(
        &mut self,
        interp: &mut SparseBackwardDriver<'ir, S, V, E, Lk, P, Sem>,
        _: DemandWork,
        (): (),
    ) -> Result<(), E> {
        // Discover the contexts the work's calls crossed into before waking
        // anything, so every changed fact's context is discovered.
        interp.inner_mut().apply_activations()?;
        interp.wake_changed()
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
            driver: StandardFixpointInterpreter::new(transfer, DemandCallers::new()),
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
        let transfer = self.driver.inner();
        transfer
            .env
            .contexts()
            .filter(|(_, env)| transfer.contexts.contains_key(env))
            .map(|(key, _)| key)
    }

    /// The `(target stage, callable body)` a discovered context analyzes.
    pub fn scope(&self, context: &P::Key) -> Option<BodyScope> {
        let transfer = self.driver.inner();
        let context = transfer.contexts.get(&transfer.discovered(context)?)?;
        Some((context.target.stage, context.body))
    }

    /// The converged demand fact for `value` in `context`.
    pub fn fact(&self, context: &P::Key, value: impl Into<SSAValue>) -> Option<&V> {
        let transfer = self.driver.inner();
        let env = transfer.discovered(context)?;
        transfer.env.environment(env).ok()?.get(value.into())
    }

    /// The joined demand on operand `index` of every return site of `context`.
    pub fn result_value(&self, context: &P::Key, index: usize) -> Option<V> {
        let transfer = self.driver.inner();
        let env = transfer.discovered(context)?;
        let operands = transfer.contexts.get(&env)?.outputs.as_ref()?.get(index)?;
        operands
            .iter()
            .filter_map(|operand| self.fact(context, *operand))
            .cloned()
            .reduce(|joined, fact| joined.join(&fact))
    }

    /// All converged `(value, fact)` pairs in `context`, reading only that
    /// context's environment.
    pub fn facts<'a>(&'a self, context: &'a P::Key) -> impl Iterator<Item = (SSAValue, &'a V)> {
        let transfer = self.driver.inner();
        transfer
            .discovered(context)
            .and_then(|env| transfer.env.environment(env).ok())
            .into_iter()
            .flat_map(|facts| facts.iter())
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
    S: StageMeta + StageQuery + InterpDispatch<SparseBackwardTransfer<'ir, S, V, E, Lk, P, Sem>>,
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
        let (target, root, env) = self
            .driver
            .inner_mut()
            .submit_call(stage, &callee, outputs)?;
        self.driver.inner_mut().apply_activations()?;
        self.driver.wake_changed()?;
        self.driver
            .drain_worklist(&mut SparseBackwardWorkAnalysis)?;
        let params = self.driver.inner().entry_params(target)?;
        let facts = self.driver.inner().param_facts(env, &params)?;
        self.root = Some(root);
        Ok((facts, self))
    }
}
