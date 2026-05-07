//! Frame-based semantics for the new interpreter framework.
//!
//! This module keeps SCF traversal policy inside SCF-owned frame types while
//! leaving interpreter-specific scheduling decisions behind small dispatch
//! traits. `scf.if` and `scf.for` both execute single-block bodies by asking
//! the interpreter to create a block frame through [`ScfBlockDispatch`]. When a
//! branch condition is indeterminate, concrete interpreters report
//! [`InterpreterError::IndeterminateBranch`], while abstract interpreters may
//! choose a conservative summary or a more precise traversal strategy.
//!
//! The standard `scf.for` frame keeps its phase enum private. For custom
//! abstract interpreters that need to handle an indeterminate loop condition,
//! [`ForContinuation`] exposes the precise resumable loop state without making
//! the entire frame state machine public.

use std::hash::Hash;
use std::marker::PhantomData;

use core::convert::Infallible;

use kirin::ir::{LiftFrom, Product, TryLift, TryLiftFrom};
use kirin::prelude::{Block, CompileTimeValue, Dialect, HasStageInfo, ResultValue, SSAValue};
use kirin_interpreter_new::{
    AbstractInterpreterWithStore, AbstractValue, BlockFrame, BlockTransfer, BranchCondition,
    ConcreteBlockTransfer, ConcreteInterpreter, Env, EnvIndex, Frame, FrameEffect, HasLocation,
    Interpretable, InterpreterError, Location, ProjectOrSelf, SimpleFixpointInterpreter,
    StatementEffect, Summary,
};

use crate::{For, ForLoopValue, If, StructuredControlFlow, Yield};

type IfFrameMarker<L, T, V, X> = PhantomData<fn() -> (L, T, V, X)>;
type ForFrameMarker<L, T, X> = PhantomData<fn() -> (L, T, X)>;

pub trait ScfBlockDispatch<L: Dialect, F, E, V, T> {
    fn dispatch_scf_block(
        &mut self,
        _location: Location,
        block: Block,
        env: EnvIndex,
        args: Product<V>,
    ) -> Result<F, E>;
}

pub trait ScfIfDispatch<L: Dialect, F, C, E, V> {
    fn dispatch_indeterminate_if(
        &mut self,
        _location: Location,
        env: EnvIndex,
        then_body: Block,
        else_body: Block,
        results: Vec<ResultValue>,
    ) -> Result<FrameEffect<F, C>, E>;
}

pub trait ScfForDispatch<L: Dialect, T: CompileTimeValue, F, C, E, V, X> {
    fn dispatch_indeterminate_for(
        &mut self,
        continuation: ForContinuation<L, T, V, X>,
    ) -> Result<FrameEffect<F, C>, E>;
}

impl<'ir, S, L, F, C, E, V, T> ScfBlockDispatch<L, F, E, V, T>
    for ConcreteInterpreter<'ir, S, F, C, E, V>
where
    S: HasStageInfo<L>,
    L: Dialect,
    F: TryLiftFrom<BlockFrame<L, V, T>>,
    E: From<<F as TryLiftFrom<BlockFrame<L, V, T>>>::Error>,
    V: Clone,
{
    fn dispatch_scf_block(
        &mut self,
        location: Location,
        block: Block,
        env: EnvIndex,
        args: Product<V>,
    ) -> Result<F, E> {
        BlockFrame::<L, V, T>::new(location.stage, block, env, args)
            .try_lift()
            .map_err(E::from)
    }
}

impl<'ir, S, L, F, C, E, V> ScfIfDispatch<L, F, C, E, V> for ConcreteInterpreter<'ir, S, F, C, E, V>
where
    L: Dialect,
    E: LiftFrom<InterpreterError>,
{
    fn dispatch_indeterminate_if(
        &mut self,
        _location: Location,
        _env: EnvIndex,
        _then_body: Block,
        _else_body: Block,
        _results: Vec<ResultValue>,
    ) -> Result<FrameEffect<F, C>, E> {
        Err(E::lift_from(InterpreterError::IndeterminateBranch))
    }
}

impl<'ir, S, L, T, F, C, E, V, X> ScfForDispatch<L, T, F, C, E, V, X>
    for ConcreteInterpreter<'ir, S, F, C, E, V>
where
    L: Dialect,
    T: CompileTimeValue,
    E: LiftFrom<InterpreterError>,
    X: BlockTransfer<Value = V>,
{
    fn dispatch_indeterminate_for(
        &mut self,
        _continuation: ForContinuation<L, T, V, X>,
    ) -> Result<FrameEffect<F, C>, E> {
        Err(E::lift_from(InterpreterError::IndeterminateBranch))
    }
}

impl<'ir, S, L, F, C, E, V, T, Store> ScfBlockDispatch<L, F, E, V, T>
    for AbstractInterpreterWithStore<'ir, S, F, C, E, Store>
where
    S: HasStageInfo<L>,
    L: Dialect,
    F: TryLiftFrom<BlockFrame<L, V, T>>,
    E: From<<F as TryLiftFrom<BlockFrame<L, V, T>>>::Error>,
    V: Clone,
{
    fn dispatch_scf_block(
        &mut self,
        location: Location,
        block: Block,
        env: EnvIndex,
        args: Product<V>,
    ) -> Result<F, E> {
        BlockFrame::<L, V, T>::new(location.stage, block, env, args)
            .try_lift()
            .map_err(E::from)
    }
}

impl<'ir, S, L, T, F, C, E, V, X, Store> ScfForDispatch<L, T, F, C, E, V, X>
    for AbstractInterpreterWithStore<'ir, S, F, C, E, Store>
where
    S: HasStageInfo<L>,
    L: Dialect,
    T: CompileTimeValue,
    Store: Env<V>,
    E: LiftFrom<InterpreterError> + LiftFrom<Store::Error>,
    V: AbstractValue,
    X: BlockTransfer<Value = V>,
{
    fn dispatch_indeterminate_for(
        &mut self,
        continuation: ForContinuation<L, T, V, X>,
    ) -> Result<FrameEffect<F, C>, E> {
        let values = continuation
            .results
            .iter()
            .map(|_| V::top())
            .collect::<Product<_>>();
        write_results(
            self,
            continuation.env,
            continuation.results.as_slice(),
            values,
        )?;
        Ok(FrameEffect::Done)
    }
}

impl<'ir, Stage, K, L, F, C, E, V, T, Sum, Store> ScfBlockDispatch<L, F, E, V, T>
    for SimpleFixpointInterpreter<'ir, Stage, K, F, C, E, Sum, Store>
where
    Stage: HasStageInfo<L>,
    K: Clone + Eq + Hash,
    L: Dialect,
    Sum: Summary,
    F: TryLiftFrom<BlockFrame<L, V, T>>,
    E: From<<F as TryLiftFrom<BlockFrame<L, V, T>>>::Error>,
    V: Clone,
{
    fn dispatch_scf_block(
        &mut self,
        location: Location,
        block: Block,
        env: EnvIndex,
        args: Product<V>,
    ) -> Result<F, E> {
        BlockFrame::<L, V, T>::new(location.stage, block, env, args)
            .try_lift()
            .map_err(E::from)
    }
}

impl<'ir, S, K, L, T, F, C, E, V, X, Sum, Store> ScfForDispatch<L, T, F, C, E, V, X>
    for SimpleFixpointInterpreter<'ir, S, K, F, C, E, Sum, Store>
where
    S: HasStageInfo<L>,
    K: Clone + Eq + Hash,
    L: Dialect,
    T: CompileTimeValue,
    Sum: Summary,
    Store: Env<V>,
    E: LiftFrom<InterpreterError> + LiftFrom<<Store as Env<V>>::Error>,
    V: AbstractValue,
    X: BlockTransfer<Value = V>,
{
    fn dispatch_indeterminate_for(
        &mut self,
        continuation: ForContinuation<L, T, V, X>,
    ) -> Result<FrameEffect<F, C>, E> {
        let values = continuation
            .results
            .iter()
            .map(|_| V::top())
            .collect::<Product<_>>();
        write_results(
            self,
            continuation.env,
            continuation.results.as_slice(),
            values,
        )?;
        Ok(FrameEffect::Done)
    }
}

impl<'ir, S, L, F, C, E, V, Store> ScfIfDispatch<L, F, C, E, V>
    for AbstractInterpreterWithStore<'ir, S, F, C, E, Store>
where
    S: HasStageInfo<L>,
    L: Dialect,
    Store: Env<V>,
    E: LiftFrom<InterpreterError> + LiftFrom<Store::Error>,
    V: AbstractValue,
{
    fn dispatch_indeterminate_if(
        &mut self,
        _location: Location,
        env: EnvIndex,
        _then_body: Block,
        _else_body: Block,
        results: Vec<ResultValue>,
    ) -> Result<FrameEffect<F, C>, E> {
        let values = results.iter().map(|_| V::top()).collect::<Product<_>>();
        write_results(self, env, results.as_slice(), values)?;
        Ok(FrameEffect::Done)
    }
}

impl<'ir, S, K, L, F, C, E, V, Sum, Store> ScfIfDispatch<L, F, C, E, V>
    for SimpleFixpointInterpreter<'ir, S, K, F, C, E, Sum, Store>
where
    S: HasStageInfo<L>,
    K: Clone + Eq + Hash,
    L: Dialect,
    Sum: Summary,
    Store: Env<V>,
    E: LiftFrom<InterpreterError> + LiftFrom<<Store as Env<V>>::Error>,
    V: AbstractValue,
{
    fn dispatch_indeterminate_if(
        &mut self,
        _location: Location,
        env: EnvIndex,
        _then_body: Block,
        _else_body: Block,
        results: Vec<ResultValue>,
    ) -> Result<FrameEffect<F, C>, E> {
        let values = results.iter().map(|_| V::top()).collect::<Product<_>>();
        write_results(self, env, results.as_slice(), values)?;
        Ok(FrameEffect::Done)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScfCompletion<V> {
    Yield(Product<V>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScfFrame<L: Dialect, T: CompileTimeValue, V, X = ConcreteBlockTransfer<V>> {
    If(IfFrame<L, T, V, X>),
    For(ForFrame<L, T, V, X>),
}

impl<L: Dialect, T: CompileTimeValue, V, X> TryLiftFrom<IfFrame<L, T, V, X>>
    for ScfFrame<L, T, V, X>
{
    type Error = Infallible;

    fn try_lift_from(frame: IfFrame<L, T, V, X>) -> Result<Self, Self::Error> {
        Ok(Self::If(frame))
    }
}

impl<L: Dialect, T: CompileTimeValue, V, X> TryLiftFrom<ForFrame<L, T, V, X>>
    for ScfFrame<L, T, V, X>
{
    type Error = Infallible;

    fn try_lift_from(frame: ForFrame<L, T, V, X>) -> Result<Self, Self::Error> {
        Ok(Self::For(frame))
    }
}

impl<L: Dialect, T: CompileTimeValue, V, X> HasLocation for ScfFrame<L, T, V, X> {
    fn location(&self) -> Location {
        match self {
            Self::If(frame) => frame.location(),
            Self::For(frame) => frame.location(),
        }
    }
}

impl<I, L, F, C, E, T, V, X> Frame<I, F, C, E> for ScfFrame<L, T, V, X>
where
    L: Dialect,
    T: CompileTimeValue,
    IfFrame<L, T, V, X>: Frame<I, F, C, E>,
    ForFrame<L, T, V, X>: Frame<I, F, C, E>,
{
    fn step(self, interp: &mut I) -> Result<FrameEffect<F, C>, E> {
        match self {
            Self::If(frame) => frame.step(interp),
            Self::For(frame) => frame.step(interp),
        }
    }

    fn resume_done(self, interp: &mut I) -> Result<FrameEffect<F, C>, E> {
        match self {
            Self::If(frame) => frame.resume_done(interp),
            Self::For(frame) => frame.resume_done(interp),
        }
    }

    fn resume(self, completion: C, interp: &mut I) -> Result<FrameEffect<F, C>, E> {
        match self {
            Self::If(frame) => frame.resume(completion, interp),
            Self::For(frame) => frame.resume(completion, interp),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IfFrame<L: Dialect, T: CompileTimeValue, V, X = ConcreteBlockTransfer<V>> {
    pub location: Location,
    pub env: EnvIndex,
    condition: SSAValue,
    then_body: Block,
    else_body: Block,
    results: Vec<ResultValue>,
    phase: IfPhase,
    _marker: IfFrameMarker<L, T, V, X>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum IfPhase {
    Entry,
    Active,
}

impl<L: Dialect, T: CompileTimeValue, V, X> IfFrame<L, T, V, X> {
    fn new(location: Location, env: EnvIndex, op: &If<T>) -> Self {
        Self {
            location,
            env,
            condition: op.condition,
            then_body: op.then_body,
            else_body: op.else_body,
            results: op.results.clone(),
            phase: IfPhase::Entry,
            _marker: PhantomData,
        }
    }

    fn active(mut self) -> Self {
        self.phase = IfPhase::Active;
        self
    }
}

impl<L: Dialect, T: CompileTimeValue, V, X> HasLocation for IfFrame<L, T, V, X> {
    fn location(&self) -> Location {
        self.location
    }
}

impl<I, L, F, C, E, T, V, X> Frame<I, F, C, E> for IfFrame<L, T, V, X>
where
    I: Env<V, Error = E> + ScfBlockDispatch<L, F, E, V, X> + ScfIfDispatch<L, F, C, E, V>,
    L: Dialect,
    F: TryLiftFrom<IfFrame<L, T, V, X>>,
    C: ProjectOrSelf<ScfCompletion<V>>,
    E: LiftFrom<InterpreterError> + From<<F as TryLiftFrom<IfFrame<L, T, V, X>>>::Error>,
    T: CompileTimeValue,
    V: BranchCondition,
    X: BlockTransfer<Value = V>,
{
    fn step(self, interp: &mut I) -> Result<FrameEffect<F, C>, E> {
        match self.phase {
            IfPhase::Entry => {
                let block = match interp.read(self.env, self.condition)?.is_truthy() {
                    Some(true) => self.then_body,
                    Some(false) => self.else_body,
                    None => {
                        return interp.dispatch_indeterminate_if(
                            self.location,
                            self.env,
                            self.then_body,
                            self.else_body,
                            self.results,
                        );
                    }
                };
                let child =
                    interp.dispatch_scf_block(self.location, block, self.env, Product::new())?;
                Ok(FrameEffect::Push {
                    parent: self.active().try_lift()?,
                    child,
                })
            }
            IfPhase::Active => Err(E::lift_from(InterpreterError::UnexpectedCompletion {
                location: self.location,
                completion: "active scf.if frame stepped",
            })),
        }
    }

    fn resume_done(self, _interp: &mut I) -> Result<FrameEffect<F, C>, E> {
        Err(E::lift_from(InterpreterError::UnexpectedCompletion {
            location: self.location,
            completion: "scf.if body completed without scf.yield",
        }))
    }

    fn resume(self, completion: C, interp: &mut I) -> Result<FrameEffect<F, C>, E> {
        match completion.project_or_self() {
            Ok(ScfCompletion::Yield(value)) => {
                write_results(interp, self.env, self.results.as_slice(), value)?;
                Ok(FrameEffect::Done)
            }
            Err(completion) => Ok(FrameEffect::Complete(completion)),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ForFrame<L: Dialect, T: CompileTimeValue, V, X = ConcreteBlockTransfer<V>> {
    pub location: Location,
    pub env: EnvIndex,
    start: SSAValue,
    end: SSAValue,
    step: SSAValue,
    init_args: Vec<SSAValue>,
    body: Block,
    results: Vec<ResultValue>,
    phase: ForPhase<V>,
    _marker: ForFrameMarker<L, T, X>,
}

/// Public suspension point for an `scf.for` loop at an indeterminate condition.
///
/// A [`ForFrame`] normally checks `iv.loop_condition(&end)` to decide whether
/// to exit or push the loop body. If that condition returns `None`, the frame
/// cannot make a concrete choice. Instead, it passes a `ForContinuation` to
/// [`ScfForDispatch::dispatch_indeterminate_for`].
///
/// The continuation contains both the static frame context (`location`, `env`,
/// `body`, `results`, and original SSA slots) and the already-read runtime loop
/// state (`iv`, `end`, `step`, and `carried`). This lets custom interpreters
/// implement policies such as "write top and stop", "push the body once", or a
/// loop-specific fixpoint strategy while still reusing the standard
/// [`ForFrame`] resume logic.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ForContinuation<L: Dialect, T: CompileTimeValue, V, X = ConcreteBlockTransfer<V>> {
    /// Program location of the `scf.for` operation.
    pub location: Location,
    /// Environment activation used by the loop body and loop results.
    pub env: EnvIndex,
    /// Original SSA slot for the loop start value.
    pub start: SSAValue,
    /// Original SSA slot for the loop end value.
    pub end_value: SSAValue,
    /// Original SSA slot for the loop step value.
    pub step_value: SSAValue,
    /// Original SSA slots for loop-carried initial values.
    pub init_args: Vec<SSAValue>,
    /// Single-block loop body.
    pub body: Block,
    /// Result slots of the `scf.for` operation.
    pub results: Vec<ResultValue>,
    /// Current induction variable value.
    pub iv: V,
    /// Already-read loop end value.
    pub end: V,
    /// Already-read loop step value.
    pub step: V,
    /// Current loop-carried values.
    pub carried: Product<V>,
    _marker: ForFrameMarker<L, T, X>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum ForPhase<V> {
    Entry,
    Check {
        iv: V,
        end: V,
        step: V,
        carried: Product<V>,
    },
}

impl<L: Dialect, T: CompileTimeValue, V, X> ForContinuation<L, T, V, X> {
    fn new(frame: ForFrameParts<T>, iv: V, end: V, step: V, carried: Product<V>) -> Self {
        Self {
            location: frame.location,
            env: frame.env,
            start: frame.start,
            end_value: frame.end,
            step_value: frame.step,
            init_args: frame.init_args,
            body: frame.body,
            results: frame.results,
            iv,
            end,
            step,
            carried,
            _marker: PhantomData,
        }
    }

    /// Build the block arguments used when entering the loop body.
    ///
    /// SCF loop bodies receive the current induction variable followed by the
    /// current loop-carried values. This helper keeps that calling convention
    /// centralized so custom dispatch implementations do not duplicate it.
    pub fn body_args(&self) -> Product<V>
    where
        V: Clone,
    {
        loop_body_args(self.iv.clone(), self.carried.clone(), self.init_args.len())
    }

    /// Rebuild the standard `ForFrame` in its check/resume state.
    ///
    /// Use this when an interpreter decides to push the loop body and wants the
    /// normal `ForFrame::resume` logic to handle `scf.yield`, advance the
    /// induction variable, and return to the condition check.
    pub fn into_frame(self) -> ForFrame<L, T, V, X> {
        ForFrame {
            location: self.location,
            env: self.env,
            start: self.start,
            end: self.end_value,
            step: self.step_value,
            init_args: self.init_args,
            body: self.body,
            results: self.results,
            phase: ForPhase::Check {
                iv: self.iv,
                end: self.end,
                step: self.step,
                carried: self.carried,
            },
            _marker: PhantomData,
        }
    }
}

struct ForFrameParts<T: CompileTimeValue> {
    location: Location,
    env: EnvIndex,
    start: SSAValue,
    end: SSAValue,
    step: SSAValue,
    init_args: Vec<SSAValue>,
    body: Block,
    results: Vec<ResultValue>,
    _marker: PhantomData<T>,
}

impl<L: Dialect, T: CompileTimeValue, V, X> ForFrame<L, T, V, X> {
    fn new(location: Location, env: EnvIndex, op: &For<T>) -> Self {
        Self {
            location,
            env,
            start: op.start,
            end: op.end,
            step: op.step,
            init_args: op.init_args.clone(),
            body: op.body,
            results: op.results.clone(),
            phase: ForPhase::Entry,
            _marker: PhantomData,
        }
    }
}

impl<L: Dialect, T: CompileTimeValue, V, X> HasLocation for ForFrame<L, T, V, X> {
    fn location(&self) -> Location {
        self.location
    }
}

impl<I, L, F, C, E, T, V, X> Frame<I, F, C, E> for ForFrame<L, T, V, X>
where
    I: Env<V, Error = E> + ScfBlockDispatch<L, F, E, V, X> + ScfForDispatch<L, T, F, C, E, V, X>,
    L: Dialect,
    F: TryLiftFrom<ForFrame<L, T, V, X>>,
    C: ProjectOrSelf<ScfCompletion<V>>,
    E: LiftFrom<InterpreterError> + From<<F as TryLiftFrom<ForFrame<L, T, V, X>>>::Error>,
    T: CompileTimeValue,
    V: Clone + ForLoopValue,
    X: BlockTransfer<Value = V>,
{
    fn step(self, interp: &mut I) -> Result<FrameEffect<F, C>, E> {
        let location = self.location;
        let env = self.env;
        let start = self.start;
        let end_value = self.end;
        let step_value = self.step;
        let init_args = self.init_args;
        let body = self.body;
        let results = self.results;
        let phase = self.phase;
        let frame = ForFrameParts {
            location,
            env,
            start,
            end: end_value,
            step: step_value,
            init_args,
            body,
            results,
            _marker: PhantomData,
        };

        match phase {
            ForPhase::Entry => {
                let iv = interp.read(frame.env, frame.start)?;
                let end = interp.read(frame.env, frame.end)?;
                let step = interp.read(frame.env, frame.step)?;
                let carried = interp.read_many(frame.env, frame.init_args.as_slice())?;
                ForContinuation::<L, T, V, X>::new(frame, iv, end, step, carried)
                    .into_frame()
                    .try_lift()
                    .map(FrameEffect::Continue)
                    .map_err(E::from)
            }
            ForPhase::Check {
                iv,
                end,
                step,
                carried,
            } => match iv.loop_condition(&end) {
                Some(false) => {
                    write_results(interp, env, frame.results.as_slice(), carried)?;
                    Ok(FrameEffect::Done)
                }
                Some(true) => {
                    let continuation =
                        ForContinuation::<L, T, V, X>::new(frame, iv, end, step, carried);
                    let args = continuation.body_args();
                    let child = interp.dispatch_scf_block(
                        continuation.location,
                        continuation.body,
                        continuation.env,
                        args,
                    )?;
                    Ok(FrameEffect::Push {
                        parent: continuation.into_frame().try_lift()?,
                        child,
                    })
                }
                None => interp.dispatch_indeterminate_for(ForContinuation::<L, T, V, X>::new(
                    frame, iv, end, step, carried,
                )),
            },
        }
    }

    fn resume_done(self, _interp: &mut I) -> Result<FrameEffect<F, C>, E> {
        Err(E::lift_from(InterpreterError::UnexpectedCompletion {
            location: self.location,
            completion: "scf.for body completed without scf.yield",
        }))
    }

    fn resume(self, completion: C, _interp: &mut I) -> Result<FrameEffect<F, C>, E> {
        let location = self.location;
        let env = self.env;
        let start = self.start;
        let end_value = self.end;
        let step_value = self.step;
        let init_args = self.init_args;
        let body = self.body;
        let results = self.results;

        let ForPhase::Check { iv, end, step, .. } = self.phase else {
            return Ok(FrameEffect::Complete(completion));
        };
        match completion.project_or_self() {
            Ok(ScfCompletion::Yield(carried)) => {
                let next_iv = match iv.loop_step(&step) {
                    Some(next_iv) => next_iv,
                    None => return Err(E::lift_from(InterpreterError::LoopStepOverflow)),
                };
                Self {
                    location,
                    env,
                    start,
                    end: end_value,
                    step: step_value,
                    init_args,
                    body,
                    results,
                    phase: ForPhase::Check {
                        iv: next_iv,
                        end,
                        step,
                        carried,
                    },
                    _marker: PhantomData,
                }
                .try_lift()
                .map(FrameEffect::Continue)
                .map_err(E::from)
            }
            Err(completion) => Ok(FrameEffect::Complete(completion)),
        }
    }
}

impl<L, I, F, C, E, T, X> Interpretable<L, I, F, C, E, X> for If<T>
where
    L: Dialect,
    F: TryLiftFrom<IfFrame<L, T, X::Value, X>>,
    E: From<<F as TryLiftFrom<IfFrame<L, T, X::Value, X>>>::Error>,
    T: CompileTimeValue,
    X: BlockTransfer,
{
    fn interpret(
        &self,
        location: Location,
        env: EnvIndex,
        _interp: &mut I,
    ) -> Result<StatementEffect<F, C, X>, E> {
        IfFrame::<L, T, X::Value, X>::new(location, env, self)
            .try_lift()
            .map(StatementEffect::Push)
            .map_err(E::from)
    }
}

impl<L, I, F, C, E, T, X> Interpretable<L, I, F, C, E, X> for For<T>
where
    L: Dialect,
    F: TryLiftFrom<ForFrame<L, T, X::Value, X>>,
    E: From<<F as TryLiftFrom<ForFrame<L, T, X::Value, X>>>::Error>,
    T: CompileTimeValue,
    X: BlockTransfer,
{
    fn interpret(
        &self,
        location: Location,
        env: EnvIndex,
        _interp: &mut I,
    ) -> Result<StatementEffect<F, C, X>, E> {
        ForFrame::<L, T, X::Value, X>::new(location, env, self)
            .try_lift()
            .map(StatementEffect::Push)
            .map_err(E::from)
    }
}

impl<L, I, F, C, E, T, X> Interpretable<L, I, F, C, E, X> for Yield<T>
where
    L: Dialect,
    I: Env<X::Value, Error = E>,
    C: TryLiftFrom<ScfCompletion<X::Value>>,
    E: From<<C as TryLiftFrom<ScfCompletion<X::Value>>>::Error>,
    T: CompileTimeValue,
    X: BlockTransfer,
{
    fn interpret(
        &self,
        _location: Location,
        env: EnvIndex,
        interp: &mut I,
    ) -> Result<StatementEffect<F, C, X>, E> {
        let values = interp.read_many(env, self.values.as_slice())?;
        Ok(StatementEffect::Complete(C::try_lift_from(
            ScfCompletion::Yield(values),
        )?))
    }
}

impl<L, I, F, C, E, T, X> Interpretable<L, I, F, C, E, X> for StructuredControlFlow<T>
where
    L: Dialect,
    I: Env<X::Value, Error = E>,
    F: TryLiftFrom<IfFrame<L, T, X::Value, X>> + TryLiftFrom<ForFrame<L, T, X::Value, X>>,
    C: TryLiftFrom<ScfCompletion<X::Value>>,
    E: From<<F as TryLiftFrom<IfFrame<L, T, X::Value, X>>>::Error>
        + From<<F as TryLiftFrom<ForFrame<L, T, X::Value, X>>>::Error>
        + From<<C as TryLiftFrom<ScfCompletion<X::Value>>>::Error>,
    T: CompileTimeValue,
    X: BlockTransfer,
{
    fn interpret(
        &self,
        location: Location,
        env: EnvIndex,
        interp: &mut I,
    ) -> Result<StatementEffect<F, C, X>, E> {
        match self {
            StructuredControlFlow::If(op) => {
                <If<T> as Interpretable<L, I, F, C, E, X>>::interpret(op, location, env, interp)
            }
            StructuredControlFlow::For(op) => {
                <For<T> as Interpretable<L, I, F, C, E, X>>::interpret(op, location, env, interp)
            }
            StructuredControlFlow::Yield(op) => {
                <Yield<T> as Interpretable<L, I, F, C, E, X>>::interpret(op, location, env, interp)
            }
        }
    }
}

fn loop_body_args<V: Clone>(iv: V, carried: Product<V>, init_arg_count: usize) -> Product<V> {
    let mut args = Vec::with_capacity(1 + init_arg_count);
    args.push(iv);
    args.extend(carried.into_iter().take(init_arg_count));
    Product::from_vec(args)
}

fn write_results<I, V>(
    interp: &mut I,
    env: EnvIndex,
    results: &[ResultValue],
    value: Product<V>,
) -> Result<(), I::Error>
where
    I: Env<V>,
    I::Error: LiftFrom<InterpreterError>,
{
    let results = results
        .iter()
        .copied()
        .map(SSAValue::from)
        .collect::<Vec<_>>();
    interp.write_product(env, results.as_slice(), value)
}
