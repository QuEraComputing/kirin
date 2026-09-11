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
//! (`ClassicLiveness`), not this one's.
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
//!   worklist, and the analysis scope.
//!
//! # Owners are values; scheduling is demand propagation
//!
//! `SummaryKey = Scoped<BodyScope, SSAValue>`: the fact anchor is
//! the owner. The driver's *default* self-dependent index
//! ([`OwnerSummaryDeps`]) is exactly demand propagation — a value whose fact
//! rises is rescheduled, and analyzing a value means dispatching the rules
//! that can translate its demand:
//!
//! - a statement **result** → the defining statement's backward rule;
//! - a **block argument** → its directly owning structured statement, or each
//!   indexed CFG predecessor block's terminator;
//! - a graph **port** → the statement owning the graph boundary.
//!
//! Rules read converged facts ([`DemandInterp::is_demanded`]) and raise new
//! demands ([`DemandInterp::demand`], strong liveness's spelling of the
//! shape-generic [`SparseBackwardInterp::raise_fact`]); each rule returns the
//! facts it raised as its [`SparseBackwardEffect`]. All fact mutation flows
//! through the driver's single merge path. Facts only rise in a finite-height
//! lattice, so the fixpoint terminates with O(predecessors) rule runs per rise —
//! no block re-walks, no widening, and no frames for structured control:
//! loop-carried demand (e.g. `scf.for`) converges through the value worklist.

use std::collections::{HashSet, VecDeque};
use std::marker::PhantomData;
use std::mem;

use kirin_ir::{
    Block, CompileStage, HasArguments, HasBottom, HasResults, HasTop, IsPure, Lattice, Pipeline,
    SSAKind, SSAValue, StageMeta, Statement,
};

use crate::core::query;
use crate::{
    AbstractInterpreter, Body, EnvIndex, FixpointProfile, Frame, FrameEffect, Interp,
    InterpDispatch, InterpLocation, InterpreterError, OwnerSemantics, OwnerSummaryDeps, Scoped,
    SparseBackwardSemantic, SparseStore, StageQuery, StandardFixpointInterpreter, StrongDemand,
    Summary, SummaryEffect, TerminatorArgs,
};

/// The scope a body-level backward analysis qualifies its facts with.
///
/// Arena ids are per-stage, so the stage is part of the scope; analyzing two
/// bodies in one engine keeps their facts distinct.
pub type BodyScope = (CompileStage, Body);

// ===========================================================================
// Effect + dialect-facing trait
// ===========================================================================

/// What a backward rule produced: the demand facts it raised.
///
/// Rules raise facts imperatively through
/// [`raise_fact`](SparseBackwardInterp::raise_fact) (or its [`DemandInterp`]
/// spelling [`demand`](DemandInterp::demand)) and finish with
/// [`effect()`](SparseBackwardInterp::effect), which drains the buffer into
/// this value; the driver applies it through its single merge path.
pub enum SparseBackwardEffect<V> {
    /// Demand facts to merge (possibly empty).
    Demands(Vec<(SSAValue, V)>),
}

/// [`SparseBackwardShape`](crate::SparseBackwardShape)-engine flavor: the
/// *shape-generic* fact mechanics plus [`SparseBackwardEffect`]. Any
/// sparse-backward semantics ([`StrongDemand`] today, downstream keys
/// tomorrow) shares this surface: read a converged per-value fact, raise a
/// fact, drain the raised facts into the rule's effect, and query the block
/// boundary facts terminator/structured rules map across. Rules are
/// scope-blind: they name bare SSA values, and the engine qualifies them with
/// the current analysis scope.
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
// SparseBackwardTransfer — the summary-free Interp delegate
// ===========================================================================

/// The summary-free transfer of the sparse backward engine: pipeline access,
/// the real dispatch location, and the per-rule demand buffer.
pub struct SparseBackwardTransfer<'ir, S: StageMeta, V, E, Sem = StrongDemand> {
    pipeline: &'ir Pipeline<S>,
    location: Option<InterpLocation>,
    /// Facts the currently dispatched rule has raised.
    demands: Vec<(SSAValue, V)>,
    /// The analysis's single activation: sparse backward facts live in one
    /// scope-qualified store, not per-call activation records.
    activation: EnvIndex,
    _marker: PhantomData<fn() -> (E, Sem)>,
}

impl<'ir, S: StageMeta, V, E, Sem> SparseBackwardTransfer<'ir, S, V, E, Sem> {
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

impl<'ir, S, V, E, Sem> Interp for SparseBackwardTransfer<'ir, S, V, E, Sem>
where
    S: StageMeta,
    V: Clone,
    E: From<InterpreterError>,
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

impl<'ir, S, V, E, Sem> AbstractInterpreter for SparseBackwardTransfer<'ir, S, V, E, Sem>
where
    S: StageMeta,
    V: Clone,
    E: From<InterpreterError>,
    Sem: SparseBackwardSemantic,
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

impl<'ir, S, V, E, Sem> FixpointProfile<SparseBackwardTransfer<'ir, S, V, E, Sem>>
    for SparseBackwardProfile<V, E>
where
    S: StageMeta,
    V: Clone + PartialEq + Lattice,
    E: From<InterpreterError>,
    Sem: SparseBackwardSemantic,
{
    type SummaryKey = Scoped<BodyScope, SSAValue>;
    type Summary = DemandSummary<V>;
    type Frame = DemandFrame<V>;
    type Completion = Vec<(SSAValue, V)>;
}

/// Analysis-local state carried in the driver's `store` slot: the scope facts
/// are qualified with.
#[derive(Default)]
pub struct BackwardAnalysisState {
    scope: Option<BodyScope>,
}

/// The sparse backward driver: a [`StandardFixpointInterpreter`] over
/// [`SparseBackwardTransfer`] with scope-qualified value owners and the
/// default self-dependent index (demand propagation).
pub type SparseBackwardDriver<'ir, S, V, E, Sem = StrongDemand> = StandardFixpointInterpreter<
    SparseBackwardTransfer<'ir, S, V, E, Sem>,
    SparseBackwardProfile<V, E>,
    BackwardAnalysisState,
    OwnerSummaryDeps<Scoped<BodyScope, SSAValue>>,
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

// This engine has one leaf continuation and never pushes a child, so its frame
// is also its complete stack-item composition.
impl<'ir, S, V, E, Sem> Frame<SparseBackwardDriver<'ir, S, V, E, Sem>> for DemandFrame<V>
where
    S: StageMeta + StageQuery + InterpDispatch<SparseBackwardDriver<'ir, S, V, E, Sem>>,
    V: Clone + PartialEq + Lattice + HasBottom,
    E: From<InterpreterError>,
    Sem: SparseBackwardSemantic,
{
    type Completion = Vec<(SSAValue, V)>;

    fn step_into(
        mut self,
        interp: &mut SparseBackwardDriver<'ir, S, V, E, Sem>,
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

    fn resume_done_into(
        self,
        _interp: &mut SparseBackwardDriver<'ir, S, V, E, Sem>,
    ) -> Result<FrameEffect<Self, Self::Completion>, E> {
        Err(E::from(InterpreterError::Custom(
            "demand frames never push children",
        )))
    }

    fn resume_into(
        self,
        _completion: Self::Completion,
        _interp: &mut SparseBackwardDriver<'ir, S, V, E, Sem>,
    ) -> Result<FrameEffect<Self, Self::Completion>, E> {
        Err(E::from(InterpreterError::Custom(
            "demand frames never push children",
        )))
    }
}

// ===========================================================================
// Driver capabilities: dispatch + the dialect-facing trait
// ===========================================================================

impl<'ir, S, V, E, Sem> SparseBackwardDriver<'ir, S, V, E, Sem>
where
    S: StageMeta + StageQuery + InterpDispatch<Self>,
    V: Clone + PartialEq + Lattice,
    E: From<InterpreterError>,
    Sem: SparseBackwardSemantic,
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

impl<'ir, S, V, E, Sem> SparseBackwardInterp for SparseBackwardDriver<'ir, S, V, E, Sem>
where
    S: StageMeta + StageQuery,
    V: Clone + PartialEq + Lattice + HasBottom,
    E: From<InterpreterError>,
    Sem: SparseBackwardSemantic,
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
}

// ===========================================================================
// Owner semantics: analyzing one value = running its translating rules
// ===========================================================================

struct SparseBackwardSemantics;

impl<'ir, S, V, E, Sem>
    OwnerSemantics<
        SparseBackwardDriver<'ir, S, V, E, Sem>,
        Scoped<BodyScope, SSAValue>,
        DemandSummary<V>,
        DemandFrame<V>,
        Vec<(SSAValue, V)>,
        E,
    > for SparseBackwardSemantics
where
    S: StageMeta + StageQuery + InterpDispatch<SparseBackwardDriver<'ir, S, V, E, Sem>>,
    V: Clone + PartialEq + Lattice + HasBottom,
    E: From<InterpreterError>,
    Sem: SparseBackwardSemantic,
{
    fn bottom_summary(
        &mut self,
        _interp: &mut SparseBackwardDriver<'ir, S, V, E, Sem>,
        _owner: &Scoped<BodyScope, SSAValue>,
    ) -> Result<DemandSummary<V>, E> {
        Ok(DemandSummary(V::bottom()))
    }

    fn entry_frame(
        &mut self,
        interp: &mut SparseBackwardDriver<'ir, S, V, E, Sem>,
        owner: &Scoped<BodyScope, SSAValue>,
        _summary: &DemandSummary<V>,
    ) -> Result<DemandFrame<V>, E> {
        let (stage, _cfg) = owner.scope;
        let kind = query::value_kind(interp.inner().pipeline(), stage, owner.item)?;
        let work = match kind {
            SSAKind::Result(statement, _) => vec![statement],
            SSAKind::BlockArgument(block, _) => {
                query::block_argument_predecessors(interp.inner().pipeline(), stage, block)?
                    .into_vec()
            }
            SSAKind::Port(parent, _) => vec![query::graph_port_owner(
                interp.inner().pipeline(),
                stage,
                parent,
            )?],
        };
        Ok(DemandFrame::new(stage, work))
    }

    fn complete_owner(
        &mut self,
        _interp: &mut SparseBackwardDriver<'ir, S, V, E, Sem>,
        owner: Scoped<BodyScope, SSAValue>,
        completion: Vec<(SSAValue, V)>,
    ) -> Result<SummaryEffect<Scoped<BodyScope, SSAValue>, DemandSummary<V>>, E> {
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
/// analysis.analyze(stage, cfg)?;
/// let demanded = analysis.is_demanded(stage, cfg, value);
/// ```
pub struct SparseBackwardInterpreter<'ir, S: StageMeta, V, E = InterpreterError, Sem = StrongDemand>
where
    V: Clone + PartialEq + Lattice,
    E: From<InterpreterError>,
    Sem: SparseBackwardSemantic,
{
    driver: SparseBackwardDriver<'ir, S, V, E, Sem>,
}

impl<'ir, S, V, E, Sem> SparseBackwardInterpreter<'ir, S, V, E, Sem>
where
    S: StageMeta,
    V: Clone + PartialEq + Lattice,
    E: From<InterpreterError>,
    Sem: SparseBackwardSemantic,
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

    /// The converged demand fact for `value` under the `(stage, body)` scope.
    pub fn fact(
        &self,
        stage: CompileStage,
        body: impl Into<Body>,
        value: impl Into<SSAValue>,
    ) -> Option<&V> {
        self.driver
            .summary(&Scoped::new((stage, body.into()), value.into()))
            .map(|summary| &summary.0)
    }

    /// All converged `(value, fact)` pairs under the `(stage, body)` scope.
    pub fn facts(
        &self,
        stage: CompileStage,
        body: impl Into<Body>,
    ) -> impl Iterator<Item = (SSAValue, &V)> {
        let scope = (stage, body.into());
        self.driver
            .summaries()
            .iter()
            .filter(move |(owner, _)| owner.scope == scope)
            .map(|(owner, summary)| (owner.item, &summary.0))
    }

    /// The converged facts under the `(stage, body)` scope as a
    /// [`SparseStore`] (the sparse per-SSA-value fact view; absent = bottom).
    pub fn fact_store(&self, stage: CompileStage, body: impl Into<Body>) -> SparseStore<V> {
        let mut store = SparseStore::new();
        for (value, fact) in self.facts(stage, body) {
            store.set(value, fact.clone());
        }
        store
    }
}

impl<'ir, S, V, E, Sem> SparseBackwardInterpreter<'ir, S, V, E, Sem>
where
    S: StageMeta + StageQuery + InterpDispatch<SparseBackwardDriver<'ir, S, V, E, Sem>>,
    V: Clone + PartialEq + Lattice + HasBottom,
    E: From<InterpreterError>,
    Sem: SparseBackwardSemantic,
{
    /// Run the demand fixpoint over `body` in `stage`.
    ///
    /// **Prepass**: walk the body's containment hierarchy, running every
    /// statement's rule once with nothing demanded — impure statements and
    /// terminators contribute the demand roots.
    /// **Propagation**: drain the value worklist; each risen value dispatches
    /// the rules that translate its demand.
    pub fn analyze(&mut self, stage: CompileStage, body: impl Into<Body>) -> Result<(), E> {
        let body = body.into();
        let scope = (stage, body);
        *self.driver.store_mut() = BackwardAnalysisState { scope: Some(scope) };

        let mut semantics = SparseBackwardSemantics;

        // Prepass: visit each contained body part once and collect all demand
        // roots before merging any of them, so every rule observes bottom.
        let mut bodies = VecDeque::from([body]);
        let mut visited = HashSet::new();
        let mut seeds: Vec<(SSAValue, V)> = Vec::new();
        while let Some(body) = bodies.pop_front() {
            if !visited.insert(body) {
                continue;
            }
            let contents = query::body_contents(self.driver.inner().pipeline(), stage, body)?;
            bodies.extend(contents.children);
            for statement in contents.statements {
                let SparseBackwardEffect::Demands(demands) =
                    self.driver.run_statement(stage, statement)?;
                seeds.extend(demands);
            }
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
        body: impl Into<Body>,
        value: impl Into<SSAValue>,
    ) -> bool
    where
        V: HasBottom,
    {
        self.fact(stage, body, value)
            .is_some_and(|fact| *fact != V::bottom())
    }
}
