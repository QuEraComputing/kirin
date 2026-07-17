//! Interpretation for structured control flow.
//!
//! SCF traversal is **dialect-local**: the framework owns no "scope" concept.
//! Each structured `scf` operation owns its traversal in an SCF frame, built
//! per-engine through a small dispatch capability and pushed with
//! [`SparseForwardEffect::Push`]:
//!
//! - `scf.if` -> [`ScfIfFrame`] (concrete) / [`AbstractScfIfFrame`] (abstract),
//!   via [`ScfIfDispatch`]. The frame chooses the arm (concrete) or explores
//!   both arms and joins their results (abstract).
//! - `scf.for` -> [`ScfForFrame`] / [`AbstractScfForFrame`], via [`ScfForDispatch`].
//!
//! Both reuse the framework's generic [`BodyFrame`]/[`AbstractBlockFrame`] to
//! *walk* a chosen body block — those are reusable building blocks, not
//! framework-owned structured semantics — but the structured *decision* and
//! result binding are owned by the SCF frame. A language that uses `scf`
//! composes a total frame type embedding these via [`BuildScfIf`]/[`BuildScfFor`]
//! (and the abstract equivalents [`BuildAbstractScfIf`]/[`BuildAbstractScfFor`]).

use std::collections::VecDeque;
use std::marker::PhantomData;

use kirin::prelude::Lattice;
use kirin::prelude::{Block, CompileStage, CompileTimeValue, HasBottom, Product, SSAValue};
use kirin_interpreter::dialect::{
    BranchCondition, ClassicLiveness, ClassicLivenessInterp, DemandInterp, DenseBackwardEffect,
    ForwardEval, Interpretable, InterpreterError, SparseForwardEffect, SparseForwardInterp,
    StrongDemand,
};
use kirin_interpreter::{
    AbstractBlockFrame, AbstractCompletion, AbstractFrameBuild, AbstractFrameDriver, BodyFrame,
    CallContext, Completion, ConcreteInterpreter, DenseBackwardCompletion,
    DenseBackwardFrameDriver, DenseBlockFrame, DenseFrameBuild, EnvIndex, FrameBuild, FrameDriver,
    FrameEffect, PointFacts, SparseForwardTransfer,
};

use crate::{For, ForLoopValue, If, Yield};

// ===========================================================================
// scf.if — push a dialect-owned if frame
// ===========================================================================

impl<I, T> Interpretable<I, ForwardEval> for If<T>
where
    I: SparseForwardInterp + ScfIfDispatch,
    I::Value: BranchCondition,
    T: CompileTimeValue,
{
    fn interpret(&self, interp: &mut I) -> Result<I::Effect, I::Error> {
        let stage = interp.stage();
        let index = interp.index();
        let results: Product<SSAValue> = self.results.iter().copied().map(Into::into).collect();
        // The decision is value-domain (decided concretely, undecided as `None`
        // under abstract interpretation); the SCF if frame owns what to do with
        // it — pick an arm or explore both and join.
        let decided = interp.read(self.condition)?.is_truthy();
        let frame = interp.scf_if_frame(stage, index, self.then_body, self.else_body, decided)?;
        Ok(SparseForwardEffect::Push { frame, results })
    }
}

// ===========================================================================
// scf.for — push a dialect-owned loop frame
// ===========================================================================

impl<I, T> Interpretable<I, ForwardEval> for For<T>
where
    I: SparseForwardInterp + ScfForDispatch,
    I::Value: ForLoopValue + Clone + 'static,
    T: CompileTimeValue,
{
    fn interpret(&self, interp: &mut I) -> Result<I::Effect, I::Error> {
        let stage = interp.stage();
        let index = interp.index();
        let induction = interp.read(self.start)?;
        let carried = interp.read_many(self.init_args.as_slice())?;
        let results: Product<SSAValue> = self.results.iter().copied().map(Into::into).collect();
        let frame = interp.scf_for_frame(
            stage,
            index,
            self.body,
            induction,
            self.end,
            self.step,
            carried,
            self.results.len(),
        )?;
        Ok(SparseForwardEffect::Push { frame, results })
    }
}

impl<I, T> Interpretable<I, ForwardEval> for Yield<T>
where
    I: SparseForwardInterp,
    T: CompileTimeValue,
{
    fn interpret(&self, interp: &mut I) -> Result<I::Effect, I::Error> {
        Ok(SparseForwardEffect::Yield(
            interp.read_many(self.values.as_slice())?,
        ))
    }
}

// ===========================================================================
// Backward demand (sparse) — rules only, no frames
// ===========================================================================
//
// Demand converges value-by-value on the sparse backward engine's worklist,
// so structured bodies need no walk and loops need no frame fixpoint: this
// rule re-runs whenever a result or a body block parameter it feeds rises
// (the owning statement is the body's *feeder* in the cfg topology).

/// Backward demand for `scf.if`: the condition is an unconditional control
/// root (consistent with `cf.cond_br`); a body's yield slot is demanded iff
/// the matching result is demanded.
impl<I, T> Interpretable<I, StrongDemand> for If<T>
where
    I: DemandInterp,
    I::Value: HasBottom + PartialEq,
    T: CompileTimeValue,
{
    fn interpret(&self, interp: &mut I) -> Result<I::Effect, I::Error> {
        interp.demand(self.condition)?;
        let then_slots = interp.terminator_args(self.then_body)?;
        let else_slots = interp.terminator_args(self.else_body)?;
        for (index, result) in self.results.iter().enumerate() {
            if interp.is_demanded(*result)? {
                if let Some(slot) = then_slots.get(index) {
                    interp.demand(*slot)?;
                }
                if let Some(slot) = else_slots.get(index) {
                    interp.demand(*slot)?;
                }
            }
        }
        Ok(interp.effect())
    }
}

/// Backward demand for `scf.for`: the loop bounds are unconditional control
/// roots. Carried slot `i` (result `i` / carried body parameter `i` / yield
/// slot `i` / `init_args[i]`) is demanded when the result is demanded (the
/// zero-iteration case flows `init_args[i]` straight to the result) or when
/// the carried parameter is demanded inside the body; either demands both the
/// initial value and the yield slot that feeds the next iteration.
impl<I, T> Interpretable<I, StrongDemand> for For<T>
where
    I: DemandInterp,
    I::Value: HasBottom + PartialEq,
    T: CompileTimeValue,
{
    fn interpret(&self, interp: &mut I) -> Result<I::Effect, I::Error> {
        interp.demand(self.start)?;
        interp.demand(self.end)?;
        interp.demand(self.step)?;
        // Body parameters: [induction variable, carried...].
        let params = interp.block_params(self.body)?;
        let yields = interp.terminator_args(self.body)?;
        for (index, init) in self.init_args.iter().enumerate() {
            let result_demanded = match self.results.get(index) {
                Some(result) => interp.is_demanded(*result)?,
                None => false,
            };
            let carried_demanded = match params.get(index + 1) {
                Some(param) => interp.is_demanded(*param)?,
                None => false,
            };
            if result_demanded || carried_demanded {
                interp.demand(*init)?;
                if let Some(slot) = yields.get(index) {
                    interp.demand(*slot)?;
                }
            }
        }
        Ok(interp.effect())
    }
}

/// Backward demand for `scf.yield`: inert — its operands are demanded by the
/// owning `scf.if`/`scf.for` rule, which knows the result/slot correspondence.
impl<I, T> Interpretable<I, StrongDemand> for Yield<T>
where
    I: DemandInterp,
    T: CompileTimeValue,
{
    fn interpret(&self, interp: &mut I) -> Result<I::Effect, I::Error> {
        Ok(interp.effect())
    }
}

// ===========================================================================
// Backward per-point liveness (dense) — dialect-owned dense frames
// ===========================================================================
//
// Classic per-point liveness needs the sets *inside* structured bodies, so
// scf owns dense backward frames (the backward analogue of the forward SCF
// frames). There is only one dense backward engine, so no per-engine dispatch
// trait is needed: the rules construct the frames directly through the
// `BuildDenseScf*` composition traits on the engine's total frame type.

/// Classic per-point liveness for `scf.if`: kill the results, gen the
/// condition (a use), and push a frame that walks both arms from the saved
/// after-state and joins their entry states.
impl<I, T> Interpretable<I, ClassicLiveness> for If<T>
where
    I: ClassicLivenessInterp,
    I::Frame: BuildDenseScfIf<I::Value, I::Error>,
    T: CompileTimeValue,
{
    fn interpret(&self, interp: &mut I) -> Result<I::Effect, I::Error> {
        let stage = interp.stage();
        for result in &self.results {
            interp.kill_def(*result)?;
        }
        interp.gen_live(self.condition)?;
        Ok(DenseBackwardEffect::Push {
            frame: I::Frame::scf_if(DenseScfIfFrame::new(stage, self.then_body, self.else_body)),
        })
    }
}

/// Classic per-point liveness for `scf.for`: kill the results, gen **all**
/// operand uses (bounds and initial carried values — classic liveness gens
/// uses unconditionally), and push the loop frame that iterates the body walk
/// to its loop-carried fixpoint.
impl<I, T> Interpretable<I, ClassicLiveness> for For<T>
where
    I: ClassicLivenessInterp,
    I::Frame: BuildDenseScfFor<I::Value, I::Error>,
    T: CompileTimeValue,
{
    fn interpret(&self, interp: &mut I) -> Result<I::Effect, I::Error> {
        let stage = interp.stage();
        for result in &self.results {
            interp.kill_def(*result)?;
        }
        interp.gen_live(self.start)?;
        interp.gen_live(self.end)?;
        interp.gen_live(self.step)?;
        for init in &self.init_args {
            interp.gen_live(*init)?;
        }
        Ok(DenseBackwardEffect::Push {
            frame: I::Frame::scf_for(DenseScfForFrame::new(stage, self.body)),
        })
    }
}

/// Classic per-point liveness for `scf.yield`: its operands are plain uses
/// inside the body walk.
impl<I, T> Interpretable<I, ClassicLiveness> for Yield<T>
where
    I: ClassicLivenessInterp,
    T: CompileTimeValue,
{
    fn interpret(&self, interp: &mut I) -> Result<I::Effect, I::Error> {
        for value in &self.values {
            interp.gen_live(*value)?;
        }
        Ok(DenseBackwardEffect::Next)
    }
}

/// Construction trait letting a total dense backward frame enum embed the
/// scf.if dense frame.
pub trait BuildDenseScfIf<V, E>: Sized {
    fn scf_if(frame: DenseScfIfFrame<V, E>) -> Self;
}

/// Construction trait letting a total dense backward frame enum embed the
/// scf.for dense frame.
pub trait BuildDenseScfFor<V, E>: Sized {
    fn scf_for(frame: DenseScfForFrame<V, E>) -> Self;
}

/// Walk both arms of an `scf.if` from the saved after-state and join their
/// entry states: `before = gen(cond) ∪ T_then(after) ∪ T_else(after)` (the
/// rule already applied the kills and the condition gen).
pub struct DenseScfIfFrame<V, E> {
    stage: CompileStage,
    arms: VecDeque<Block>,
    /// The state after the `scf.if` (captured on the first step).
    after: Option<V>,
    /// Join of the walked arms' entry states.
    joined: Option<V>,
    _marker: PhantomData<fn() -> E>,
}

impl<V, E> DenseScfIfFrame<V, E> {
    pub fn new(stage: CompileStage, then_body: Block, else_body: Block) -> Self {
        Self {
            stage,
            arms: VecDeque::from([then_body, else_body]),
            after: None,
            joined: None,
            _marker: PhantomData,
        }
    }

    pub fn step_into<I, F>(
        mut self,
        interp: &mut I,
    ) -> Result<FrameEffect<F, DenseBackwardCompletion<V>>, E>
    where
        I: DenseBackwardFrameDriver<Value = V, Error = E, Frame = F>,
        F: DenseFrameBuild<V, E> + BuildDenseScfIf<V, E>,
        V: Clone + Lattice,
        E: From<InterpreterError>,
    {
        if self.after.is_none() {
            self.after = Some(interp.state());
        }
        match self.arms.pop_front() {
            Some(arm) => {
                let after = self.after.clone().expect("after-state captured");
                interp.replace_state(after);
                let stage = self.stage;
                Ok(FrameEffect::Push {
                    parent: F::scf_if(self),
                    child: F::from_block(DenseBlockFrame::structured_body(stage, arm)),
                })
            }
            None => {
                let joined = self
                    .joined
                    .take()
                    .unwrap_or_else(|| self.after.clone().expect("after-state captured"));
                interp.replace_state(joined);
                Ok(FrameEffect::Complete(DenseBackwardCompletion::Structured))
            }
        }
    }

    pub fn resume_into<I, F>(
        mut self,
        completion: DenseBackwardCompletion<V>,
        interp: &mut I,
    ) -> Result<FrameEffect<F, DenseBackwardCompletion<V>>, E>
    where
        I: DenseBackwardFrameDriver<Value = V, Error = E, Frame = F>,
        F: DenseFrameBuild<V, E> + BuildDenseScfIf<V, E>,
        V: Clone + Lattice,
        E: From<InterpreterError>,
    {
        match completion {
            DenseBackwardCompletion::Structured => {
                let arm_entry = interp.state();
                self.joined = Some(match self.joined.take() {
                    Some(joined) => joined.join(&arm_entry),
                    None => arm_entry,
                });
                self.step_into(interp)
            }
            DenseBackwardCompletion::Block { .. } => Err(E::from(InterpreterError::Custom(
                "an scf.if arm completed as a CFG block owner",
            ))),
        }
    }

    pub fn resume_done_into<F>(self) -> Result<FrameEffect<F, DenseBackwardCompletion<V>>, E>
    where
        E: From<InterpreterError>,
    {
        Err(E::from(InterpreterError::Custom(
            "scf.if dense frames resume only with completions",
        )))
    }
}

/// Iterate the `scf.for` body walk to its loop-carried fixpoint:
/// `E* = seed ∪ carry(walk(E*))` where `seed` is the state after the loop
/// (kills/gens already applied by the rule) and `carry` maps a live carried
/// parameter to the yield slot feeding it. Finalizes by killing the body
/// parameters — the initial-value gens were applied by the rule.
pub struct DenseScfForFrame<V, E> {
    stage: CompileStage,
    body: Block,
    /// Captured on the first step.
    seed: Option<V>,
    params: Vec<SSAValue>,
    yields: Vec<SSAValue>,
    /// The current body-exit state estimate.
    entry: Option<V>,
    _marker: PhantomData<fn() -> E>,
}

impl<V, E> DenseScfForFrame<V, E> {
    pub fn new(stage: CompileStage, body: Block) -> Self {
        Self {
            stage,
            body,
            seed: None,
            params: Vec::new(),
            yields: Vec::new(),
            entry: None,
            _marker: PhantomData,
        }
    }

    fn carry(&self, body_entry: &V) -> V
    where
        V: Clone + Lattice + PointFacts,
    {
        let mut next = self.seed.clone().expect("seed captured");
        for (index, param) in self.params.iter().skip(1).enumerate() {
            if body_entry.contains(*param)
                && let Some(slot) = self.yields.get(index)
            {
                next.insert(*slot);
            }
        }
        next
    }

    pub fn step_into<I, F>(
        mut self,
        interp: &mut I,
    ) -> Result<FrameEffect<F, DenseBackwardCompletion<V>>, E>
    where
        I: DenseBackwardFrameDriver<Value = V, Error = E, Frame = F>,
        F: DenseFrameBuild<V, E> + BuildDenseScfFor<V, E>,
        V: Clone + PartialEq + Lattice + PointFacts,
        E: From<InterpreterError>,
    {
        if self.seed.is_none() {
            self.seed = Some(interp.state());
            self.params = interp.block_params(self.stage, self.body)?;
            self.yields = interp.terminator_args(self.stage, self.body)?;
            self.entry = self.seed.clone();
        }
        let entry = self.entry.clone().expect("entry estimate present");
        interp.replace_state(entry);
        let (stage, body) = (self.stage, self.body);
        Ok(FrameEffect::Push {
            parent: F::scf_for(self),
            child: F::from_block(DenseBlockFrame::structured_body(stage, body)),
        })
    }

    pub fn resume_into<I, F>(
        mut self,
        completion: DenseBackwardCompletion<V>,
        interp: &mut I,
    ) -> Result<FrameEffect<F, DenseBackwardCompletion<V>>, E>
    where
        I: DenseBackwardFrameDriver<Value = V, Error = E, Frame = F>,
        F: DenseFrameBuild<V, E> + BuildDenseScfFor<V, E>,
        V: Clone + PartialEq + Lattice + PointFacts,
        E: From<InterpreterError>,
    {
        match completion {
            DenseBackwardCompletion::Structured => {
                let body_entry = interp.state();
                let next = self.carry(&body_entry);
                if Some(&next) != self.entry.as_ref() {
                    // The loop-carried estimate rose: re-walk the body from it
                    // (monotone joins over a finite set — terminates).
                    self.entry = Some(next);
                    self.step_into(interp)
                } else {
                    // Stable: the final body entry, minus the body-local
                    // parameters, is the state before the loop.
                    let mut before = body_entry;
                    for param in &self.params {
                        before.remove(*param);
                    }
                    interp.replace_state(before);
                    Ok(FrameEffect::Complete(DenseBackwardCompletion::Structured))
                }
            }
            DenseBackwardCompletion::Block { .. } => Err(E::from(InterpreterError::Custom(
                "an scf.for body completed as a CFG block owner",
            ))),
        }
    }

    pub fn resume_done_into<F>(self) -> Result<FrameEffect<F, DenseBackwardCompletion<V>>, E>
    where
        E: From<InterpreterError>,
    {
        Err(E::from(InterpreterError::Custom(
            "scf.for dense frames resume only with completions",
        )))
    }
}

// ===========================================================================
// Per-engine construction of the loop frame (the minimal "push a dialect frame"
// dispatch). Concrete and abstract engines build their own loop frame, so the
// `For` rule stays engine-blind.
// ===========================================================================

/// Capability the `scf.for` rule uses to obtain this engine's loop frame.
pub trait ScfForDispatch: SparseForwardInterp {
    #[allow(clippy::too_many_arguments)]
    fn scf_for_frame(
        &mut self,
        stage: CompileStage,
        env: EnvIndex,
        body: Block,
        induction: Self::Value,
        end: SSAValue,
        step: SSAValue,
        carried: Product<Self::Value>,
        results_arity: usize,
    ) -> Result<Self::Frame, Self::Error>;
}

/// Embed the concrete [`ScfForFrame`] into a language's total frame type.
pub trait BuildScfFor<V, E>: Sized {
    fn scf_for(frame: ScfForFrame<V, E>) -> Self;
}

/// Embed the abstract [`AbstractScfForFrame`] into a language's total abstract
/// frame type.
pub trait BuildAbstractScfFor<V, E, K>: Sized {
    fn scf_for(frame: AbstractScfForFrame<V, E, K>) -> Self;
}

impl<'ir, S, V, E, Lk, F> ScfForDispatch for ConcreteInterpreter<'ir, S, V, E, Lk, F>
where
    S: kirin::prelude::StageMeta,
    V: Clone + ForLoopValue + 'static,
    E: From<InterpreterError>,
    F: FrameBuild<V, E> + BuildScfFor<V, E>,
{
    fn scf_for_frame(
        &mut self,
        stage: CompileStage,
        env: EnvIndex,
        body: Block,
        induction: V,
        end: SSAValue,
        step: SSAValue,
        carried: Product<V>,
        _results_arity: usize,
    ) -> Result<F, E> {
        Ok(F::scf_for(ScfForFrame::new(
            stage, env, body, induction, end, step, carried,
        )))
    }
}

impl<'ir, S, V, E, Lk, P, F> ScfForDispatch for SparseForwardTransfer<'ir, S, V, E, Lk, P, F>
where
    S: kirin::prelude::StageMeta,
    V: Clone + PartialEq + ForLoopValue + HasBottom,
    E: From<InterpreterError>,
    P: CallContext<V>,
    F: AbstractFrameBuild<V, E, <P as CallContext<V>>::Key>
        + BuildAbstractScfFor<V, E, <P as CallContext<V>>::Key>,
{
    fn scf_for_frame(
        &mut self,
        stage: CompileStage,
        env: EnvIndex,
        body: Block,
        induction: V,
        end: SSAValue,
        step: SSAValue,
        carried: Product<V>,
        _results_arity: usize,
    ) -> Result<F, E> {
        Ok(F::scf_for(AbstractScfForFrame::new(
            stage, env, body, induction, end, step, carried,
        )))
    }
}

// ===========================================================================
// Per-engine construction of the if frame (mirrors the loop dispatch).
// ===========================================================================

/// Capability the `scf.if` rule uses to obtain this engine's if frame.
pub trait ScfIfDispatch: SparseForwardInterp {
    fn scf_if_frame(
        &mut self,
        stage: CompileStage,
        env: EnvIndex,
        then_body: Block,
        else_body: Block,
        decided: Option<bool>,
    ) -> Result<Self::Frame, Self::Error>;
}

/// Embed the concrete [`ScfIfFrame`] into a language's total frame type.
pub trait BuildScfIf<V, E>: Sized {
    fn scf_if(frame: ScfIfFrame<V, E>) -> Self;
}

/// Embed the abstract [`AbstractScfIfFrame`] into a language's total abstract
/// frame type.
pub trait BuildAbstractScfIf<V, E, K>: Sized {
    fn scf_if(frame: AbstractScfIfFrame<V, E, K>) -> Self;
}

impl<'ir, S, V, E, Lk, F> ScfIfDispatch for ConcreteInterpreter<'ir, S, V, E, Lk, F>
where
    S: kirin::prelude::StageMeta,
    V: Clone,
    E: From<InterpreterError>,
    F: FrameBuild<V, E> + BuildScfIf<V, E>,
{
    fn scf_if_frame(
        &mut self,
        stage: CompileStage,
        env: EnvIndex,
        then_body: Block,
        else_body: Block,
        decided: Option<bool>,
    ) -> Result<F, E> {
        Ok(F::scf_if(ScfIfFrame::new(
            stage, env, then_body, else_body, decided,
        )))
    }
}

impl<'ir, S, V, E, Lk, P, F> ScfIfDispatch for SparseForwardTransfer<'ir, S, V, E, Lk, P, F>
where
    S: kirin::prelude::StageMeta,
    V: Clone + PartialEq + HasBottom,
    E: From<InterpreterError>,
    P: CallContext<V>,
    F: AbstractFrameBuild<V, E, <P as CallContext<V>>::Key>
        + BuildAbstractScfIf<V, E, <P as CallContext<V>>::Key>,
{
    fn scf_if_frame(
        &mut self,
        stage: CompileStage,
        env: EnvIndex,
        then_body: Block,
        else_body: Block,
        decided: Option<bool>,
    ) -> Result<F, E> {
        Ok(F::scf_if(AbstractScfIfFrame::new(
            stage, env, then_body, else_body, decided,
        )))
    }
}

// ===========================================================================
// Concrete if frame: pick the decided arm, relay its completion.
// ===========================================================================

/// Concrete `scf.if` traversal: push the framework [`BodyFrame`] for the decided
/// arm and relay its completion to the pusher. The structured *decision* (which
/// arm) is owned here; an undecided condition is impossible under concrete
/// execution (`IndeterminateBranch`).
pub struct ScfIfFrame<V, E> {
    stage: CompileStage,
    env: EnvIndex,
    then_body: Block,
    else_body: Block,
    decided: Option<bool>,
    _marker: PhantomData<fn() -> (V, E)>,
}

impl<V, E> ScfIfFrame<V, E>
where
    V: Clone,
    E: From<InterpreterError>,
{
    pub fn new(
        stage: CompileStage,
        env: EnvIndex,
        then_body: Block,
        else_body: Block,
        decided: Option<bool>,
    ) -> Self {
        Self {
            stage,
            env,
            then_body,
            else_body,
            decided,
            _marker: PhantomData,
        }
    }

    pub fn step_into<I, F>(self, _interp: &mut I) -> Result<FrameEffect<F, Completion<V>>, E>
    where
        I: FrameDriver<Value = V, Error = E>,
        F: FrameBuild<V, E> + BuildScfIf<V, E>,
    {
        let arm = match self.decided {
            Some(true) => self.then_body,
            Some(false) => self.else_body,
            None => return Err(E::from(InterpreterError::IndeterminateBranch)),
        };
        let body = BodyFrame::block(self.stage, self.env, arm, Product::new());
        Ok(FrameEffect::Push {
            parent: F::scf_if(self),
            child: F::from_body(body),
        })
    }

    pub fn resume_done_into<F>(self) -> Result<FrameEffect<F, Completion<V>>, E> {
        Err(E::from(InterpreterError::Custom(
            "scf.if frame resumed without a body completion",
        )))
    }

    pub fn resume_into<F>(
        self,
        completion: Completion<V>,
    ) -> Result<FrameEffect<F, Completion<V>>, E> {
        // Relay the chosen arm's completion (yield-finish or function return).
        Ok(FrameEffect::Complete(completion))
    }
}

// ===========================================================================
// Abstract if frame: explore the live arm(s) and join their finish results.
// ===========================================================================

/// Abstract `scf.if` traversal: explore the decided arm, or — when the
/// condition is undecided in the value domain — both arms, joining their finish
/// results. The "both arms + join" structured behavior is owned here; the
/// framework has no alternatives concept.
pub struct AbstractScfIfFrame<V, E, K> {
    stage: CompileStage,
    env: EnvIndex,
    remaining: VecDeque<Block>,
    acc: Option<Product<V>>,
    _marker: PhantomData<fn() -> (E, K)>,
}

impl<V, E, K> AbstractScfIfFrame<V, E, K>
where
    V: Clone + PartialEq,
    E: From<InterpreterError>,
    K: Clone + Eq + std::hash::Hash,
{
    pub fn new(
        stage: CompileStage,
        env: EnvIndex,
        then_body: Block,
        else_body: Block,
        decided: Option<bool>,
    ) -> Self {
        let remaining = match decided {
            Some(true) => vec![then_body],
            Some(false) => vec![else_body],
            None => vec![then_body, else_body],
        };
        Self {
            stage,
            env,
            remaining: remaining.into(),
            acc: None,
            _marker: PhantomData,
        }
    }

    fn join_acc<I>(&mut self, interp: &mut I, values: Product<V>) -> Result<(), E>
    where
        I: AbstractFrameDriver<Value = V, Error = E>,
    {
        let merged = match self.acc.take() {
            None => values,
            Some(current) => interp.analysis_merge(&current, &values, 0)?,
        };
        self.acc = Some(merged);
        Ok(())
    }

    pub fn step_into<I, F>(
        mut self,
        _interp: &mut I,
    ) -> Result<FrameEffect<F, AbstractCompletion<V>>, E>
    where
        I: AbstractFrameDriver<Value = V, Error = E, SummaryKey = K>,
        F: AbstractFrameBuild<V, E, K> + BuildAbstractScfIf<V, E, K>,
    {
        match self.remaining.pop_front() {
            None => Ok(FrameEffect::Complete(AbstractCompletion::Finished(
                self.acc,
            ))),
            Some(block) => {
                let body = AbstractBlockFrame::new(self.stage, self.env, block, Product::new());
                Ok(FrameEffect::Push {
                    parent: F::scf_if(self),
                    child: F::from_block(body),
                })
            }
        }
    }

    pub fn resume_done_into<F>(self) -> Result<FrameEffect<F, AbstractCompletion<V>>, E> {
        Err(E::from(InterpreterError::Custom(
            "scf.if frame resumed without a body completion",
        )))
    }

    pub fn resume_into<I, F>(
        mut self,
        completion: AbstractCompletion<V>,
        interp: &mut I,
    ) -> Result<FrameEffect<F, AbstractCompletion<V>>, E>
    where
        I: AbstractFrameDriver<Value = V, Error = E, SummaryKey = K>,
        F: AbstractFrameBuild<V, E, K> + BuildAbstractScfIf<V, E, K>,
    {
        match completion {
            AbstractCompletion::Finished(Some(values)) => {
                self.join_acc(interp, values)?;
                Ok(FrameEffect::Continue(F::scf_if(self)))
            }
            // This arm returned (no finish value): skip it, try the next.
            AbstractCompletion::Finished(None) => Ok(FrameEffect::Continue(F::scf_if(self))),
            AbstractCompletion::FunctionDone => Err(E::from(InterpreterError::Custom(
                "scf.if frame resumed with a function completion",
            ))),
            AbstractCompletion::CFGBlock { .. } => Err(E::from(InterpreterError::Custom(
                "scf.if frame resumed with a CFG-block completion",
            ))),
        }
    }
}

// ===========================================================================
// Concrete loop frame: precise counted-loop traversal.
// ===========================================================================

/// Concrete `scf.for` traversal: re-push the body block while the induction
/// variable satisfies the loop condition, advancing it by `step` each turn and
/// carrying the yielded values forward. Loop policy lives here, not in the
/// framework.
pub struct ScfForFrame<V, E> {
    stage: CompileStage,
    env: EnvIndex,
    body: Block,
    induction: V,
    end: SSAValue,
    step: SSAValue,
    carried: Product<V>,
    _marker: PhantomData<fn() -> E>,
}

impl<V, E> ScfForFrame<V, E>
where
    V: Clone + ForLoopValue,
    E: From<InterpreterError>,
{
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        stage: CompileStage,
        env: EnvIndex,
        body: Block,
        induction: V,
        end: SSAValue,
        step: SSAValue,
        carried: Product<V>,
    ) -> Self {
        Self {
            stage,
            env,
            body,
            induction,
            end,
            step,
            carried,
            _marker: PhantomData,
        }
    }

    pub fn step_into<I, F>(self, interp: &mut I) -> Result<FrameEffect<F, Completion<V>>, E>
    where
        I: FrameDriver<Value = V, Error = E>,
        F: FrameBuild<V, E> + BuildScfFor<V, E>,
    {
        let end = interp.env_read(self.env, self.end)?;
        match self.induction.loop_condition(&end) {
            Some(true) => {
                let args: Product<V> = std::iter::once(self.induction.clone())
                    .chain(self.carried.iter().cloned())
                    .collect();
                let body = BodyFrame::block(self.stage, self.env, self.body, args);
                Ok(FrameEffect::Push {
                    parent: F::scf_for(self),
                    child: F::from_body(body),
                })
            }
            Some(false) => Ok(FrameEffect::Complete(Completion::Finished(self.carried))),
            None => Err(E::from(InterpreterError::IndeterminateBranch)),
        }
    }

    pub fn resume_done_into<F>(self) -> Result<FrameEffect<F, Completion<V>>, E> {
        Err(E::from(InterpreterError::Custom(
            "scf.for frame resumed without a body completion",
        )))
    }

    pub fn resume_into<I, F>(
        mut self,
        completion: Completion<V>,
        interp: &mut I,
    ) -> Result<FrameEffect<F, Completion<V>>, E>
    where
        I: FrameDriver<Value = V, Error = E>,
        F: FrameBuild<V, E> + BuildScfFor<V, E>,
    {
        match completion {
            // The body yielded: advance the induction variable and re-check.
            Completion::Finished(yielded) => {
                let step = interp.env_read(self.env, self.step)?;
                let next = self
                    .induction
                    .loop_step(&step)
                    .ok_or_else(|| E::from(InterpreterError::LoopStepOverflow))?;
                self.induction = next;
                self.carried = yielded;
                Ok(FrameEffect::Continue(F::scf_for(self)))
            }
            // A `ret` inside the body returns from the enclosing function.
            Completion::Returned(values) => Ok(FrameEffect::Complete(Completion::Returned(values))),
        }
    }
}

// ===========================================================================
// Abstract loop frame: sound over-approximation.
// ===========================================================================

/// Abstract `scf.for` traversal: a loop-carried fixpoint. The body is analyzed
/// with the current entry (`[induction] ++ carried`); each yield advances the
/// induction variable and joins the new entry state, widening after the
/// analysis threshold, until the entry is stable. Finish values join across the
/// possible exits (including the zero-iteration "skip" when the loop condition
/// is undecided). Loop policy lives here, not in the framework.
pub struct AbstractScfForFrame<V, E, K> {
    stage: CompileStage,
    env: EnvIndex,
    body: Block,
    end: SSAValue,
    step: SSAValue,
    /// Current entry state bound to the body: `[induction] ++ carried`.
    entry: Product<V>,
    /// The zero-iteration result (original carried values).
    inits: Product<V>,
    /// Joined finish values across the explored exits.
    finish: Option<Product<V>>,
    iterations: usize,
    entered: bool,
    _marker: PhantomData<fn() -> (E, K)>,
}

impl<V, E, K> AbstractScfForFrame<V, E, K>
where
    V: Clone + PartialEq + ForLoopValue,
    E: From<InterpreterError>,
    K: Clone + Eq + std::hash::Hash,
{
    pub fn new(
        stage: CompileStage,
        env: EnvIndex,
        body: Block,
        induction: V,
        end: SSAValue,
        step: SSAValue,
        carried: Product<V>,
    ) -> Self {
        let entry: Product<V> = std::iter::once(induction)
            .chain(carried.iter().cloned())
            .collect();
        Self {
            stage,
            env,
            body,
            end,
            step,
            entry,
            inits: carried,
            finish: None,
            iterations: 0,
            entered: false,
            _marker: PhantomData,
        }
    }

    fn induction(&self) -> Result<V, E> {
        self.entry.get(0).cloned().ok_or_else(|| {
            E::from(InterpreterError::Custom(
                "scf.for body is missing its induction parameter",
            ))
        })
    }

    fn join_finish<I>(&mut self, interp: &mut I, values: Product<V>) -> Result<(), E>
    where
        I: AbstractFrameDriver<Value = V, Error = E>,
    {
        let merged = match self.finish.take() {
            None => values,
            Some(current) => interp.analysis_merge(&current, &values, 0)?,
        };
        self.finish = Some(merged);
        Ok(())
    }

    pub fn step_into<I, F>(
        mut self,
        interp: &mut I,
    ) -> Result<FrameEffect<F, AbstractCompletion<V>>, E>
    where
        I: AbstractFrameDriver<Value = V, Error = E, SummaryKey = K>,
        F: AbstractFrameBuild<V, E, K> + BuildAbstractScfFor<V, E, K>,
    {
        if !self.entered {
            self.entered = true;
            let end = interp.env_read(self.env, self.end)?;
            match self.induction()?.loop_condition(&end) {
                // Loop never runs: result is the initial carried values.
                Some(false) => {
                    return Ok(FrameEffect::Complete(AbstractCompletion::Finished(Some(
                        self.inits,
                    ))));
                }
                // Undecided: the loop may run zero times — join that exit.
                None => self.finish = Some(self.inits.clone()),
                Some(true) => {}
            }
            self.iterations = 1;
        }
        let body = AbstractBlockFrame::new(self.stage, self.env, self.body, self.entry.clone());
        Ok(FrameEffect::Push {
            parent: F::scf_for(self),
            child: F::from_block(body),
        })
    }

    pub fn resume_done_into<F>(self) -> Result<FrameEffect<F, AbstractCompletion<V>>, E> {
        Err(E::from(InterpreterError::Custom(
            "scf.for frame resumed without a body completion",
        )))
    }

    pub fn resume_into<I, F>(
        mut self,
        completion: AbstractCompletion<V>,
        interp: &mut I,
    ) -> Result<FrameEffect<F, AbstractCompletion<V>>, E>
    where
        I: AbstractFrameDriver<Value = V, Error = E, SummaryKey = K>,
        F: AbstractFrameBuild<V, E, K> + BuildAbstractScfFor<V, E, K>,
    {
        let yielded = match completion {
            AbstractCompletion::Finished(Some(values)) => values,
            // The body returned: the loop finishes with what it has joined.
            AbstractCompletion::Finished(None) => {
                return Ok(FrameEffect::Complete(AbstractCompletion::Finished(
                    self.finish,
                )));
            }
            AbstractCompletion::FunctionDone => {
                return Err(E::from(InterpreterError::Custom(
                    "scf.for frame resumed with a function completion",
                )));
            }
            AbstractCompletion::CFGBlock { .. } => {
                return Err(E::from(InterpreterError::Custom(
                    "scf.for frame resumed with a CFG-block completion",
                )));
            }
        };

        let step = interp.env_read(self.env, self.step)?;
        let next = self
            .induction()?
            .loop_step(&step)
            .ok_or_else(|| E::from(InterpreterError::LoopStepOverflow))?;
        let end = interp.env_read(self.env, self.end)?;
        let next_args: Product<V> = std::iter::once(next.clone())
            .chain(yielded.iter().cloned())
            .collect();

        let (contribute, continue_loop) = match next.loop_condition(&end) {
            Some(false) => (true, false),
            Some(true) => (false, true),
            None => (true, true),
        };
        if contribute {
            self.join_finish(interp, yielded)?;
        }
        if !continue_loop {
            return Ok(FrameEffect::Complete(AbstractCompletion::Finished(
                self.finish,
            )));
        }

        let joined = interp.analysis_merge(&self.entry, &next_args, self.iterations)?;
        if joined == self.entry {
            // Entry state stable: re-running the body adds nothing.
            return Ok(FrameEffect::Complete(AbstractCompletion::Finished(
                self.finish,
            )));
        }
        self.entry = joined;
        self.iterations += 1;
        if self.iterations > interp.max_iterations() {
            return Err(E::from(InterpreterError::FixpointDiverged));
        }
        Ok(FrameEffect::Continue(F::scf_for(self)))
    }
}
