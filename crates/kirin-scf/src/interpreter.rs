//! Interpretation for structured control flow.
//!
//! SCF traversal is **dialect-local**: the framework owns no "scope" concept.
//! Each structured `scf` operation owns its traversal in an SCF frame, built
//! per-engine through a small dispatch capability and pushed with
//! [`ForwardEffect::Push`]:
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

use kirin::prelude::{Block, CompileStage, CompileTimeValue, HasBottom, Product, SSAValue};
use kirin_interpreter::dialect::{
    BranchCondition, ForwardContext, ForwardCtx, ForwardEffect, ForwardInterp, Interpretable,
    InterpreterError,
};
use kirin_interpreter::{
    AbstractBlockFrame, AbstractCompletion, AbstractFrameBuild, AbstractFrameDriver,
    AbstractInterpreter, BodyFrame, CallContext, Completion, ConcreteInterpreter, EnvIndex,
    FrameBuild, FrameDriver, FrameEffect,
};

use crate::{For, ForLoopValue, If, Yield};

// ===========================================================================
// scf.if — push a dialect-owned if frame
// ===========================================================================

impl<I, T> Interpretable<ForwardContext<'_, I>> for If<T>
where
    I: ForwardInterp + ScfIfDispatch,
    I::Value: BranchCondition,
    T: CompileTimeValue,
{
    fn interpret(&self, ctx: &mut ForwardContext<'_, I>) -> Result<I::Effect, I::Error> {
        let stage = ctx.stage();
        let env = ctx.env();
        let results: Product<SSAValue> = self.results.iter().copied().map(Into::into).collect();
        // The decision is value-domain (decided concretely, undecided as `None`
        // under abstract interpretation); the SCF if frame owns what to do with
        // it — pick an arm or explore both and join.
        let decided = ctx.read(self.condition)?.is_truthy();
        let frame =
            ctx.interp()
                .scf_if_frame(stage, env, self.then_body, self.else_body, decided)?;
        Ok(ForwardEffect::Push { frame, results })
    }
}

// ===========================================================================
// scf.for — push a dialect-owned loop frame
// ===========================================================================

impl<I, T> Interpretable<ForwardContext<'_, I>> for For<T>
where
    I: ForwardInterp + ScfForDispatch,
    I::Value: ForLoopValue + Clone + 'static,
    T: CompileTimeValue,
{
    fn interpret(&self, ctx: &mut ForwardContext<'_, I>) -> Result<I::Effect, I::Error> {
        let stage = ctx.stage();
        let env = ctx.env();
        let induction = ctx.read(self.start)?;
        let carried = ctx.read_many(self.init_args.as_slice())?;
        let results: Product<SSAValue> = self.results.iter().copied().map(Into::into).collect();
        let frame = ctx.interp().scf_for_frame(
            stage,
            env,
            self.body,
            induction,
            self.end,
            self.step,
            carried,
            self.results.len(),
        )?;
        Ok(ForwardEffect::Push { frame, results })
    }
}

impl<I, T> Interpretable<ForwardContext<'_, I>> for Yield<T>
where
    I: ForwardInterp,
    T: CompileTimeValue,
{
    fn interpret(&self, ctx: &mut ForwardContext<'_, I>) -> Result<I::Effect, I::Error> {
        Ok(ForwardEffect::Yield(ctx.read_many(self.values.as_slice())?))
    }
}

// ===========================================================================
// Per-engine construction of the loop frame (the minimal "push a dialect frame"
// dispatch). Concrete and abstract engines build their own loop frame, so the
// `For` rule stays engine-blind.
// ===========================================================================

/// Capability the `scf.for` rule uses to obtain this engine's loop frame.
pub trait ScfForDispatch: ForwardInterp {
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

impl<'ir, S, V, E, Lk, P, F> ScfForDispatch for AbstractInterpreter<'ir, S, V, E, Lk, P, F>
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
pub trait ScfIfDispatch: ForwardInterp {
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

impl<'ir, S, V, E, Lk, P, F> ScfIfDispatch for AbstractInterpreter<'ir, S, V, E, Lk, P, F>
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
