use std::marker::PhantomData;

use kirin::ir::TryLiftFrom;
use kirin::prelude::{Block, CompileTimeValue, Dialect, HasStageInfo, ResultValue, SSAValue};
use kirin_interpreter_new::{
    AbstractInterpreter, BlockFrame, BranchCondition, ConcreteInterpreter, ConcreteTransfer, Env,
    EnvIndex, Frame, FrameEffect, HasLocation, Interpretable, InterpreterError, Location,
    ProductValue, ProjectOrSelf, StatementEffect,
};

use crate::{For, ForLoopValue, If, StructuredControlFlow, Yield};

type IfFrameMarker<L, T, V> = PhantomData<fn() -> (L, T, V)>;
type ForFrameMarker<L, T> = PhantomData<fn() -> (L, T)>;

pub trait ScfBlockDispatch<L: Dialect, F, E, V> {
    fn dispatch_scf_block(
        &mut self,
        location: Location,
        block: Block,
        env: EnvIndex,
        args: Vec<V>,
    ) -> Result<F, E>;
}

impl<'ir, S, L, F, C, E, V> ScfBlockDispatch<L, F, E, V> for ConcreteInterpreter<'ir, S, F, C, E, V>
where
    S: HasStageInfo<L>,
    L: Dialect,
    F: From<BlockFrame<L, V>>,
    V: Clone,
{
    fn dispatch_scf_block(
        &mut self,
        location: Location,
        block: Block,
        env: EnvIndex,
        args: Vec<V>,
    ) -> Result<F, E> {
        Ok(BlockFrame::<L, V>::new(location.stage, block, env, args).into())
    }
}

impl<'ir, S, L, F, C, E, V> ScfBlockDispatch<L, F, E, V> for AbstractInterpreter<'ir, S, F, C, E, V>
where
    S: HasStageInfo<L>,
    L: Dialect,
    F: From<BlockFrame<L, V>>,
    V: Clone,
{
    fn dispatch_scf_block(
        &mut self,
        location: Location,
        block: Block,
        env: EnvIndex,
        args: Vec<V>,
    ) -> Result<F, E> {
        Ok(BlockFrame::<L, V>::new(location.stage, block, env, args).into())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScfCompletion<V> {
    Yield(V),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScfFrame<L: Dialect, T: CompileTimeValue, V> {
    If(IfFrame<L, T, V>),
    For(ForFrame<L, T, V>),
}

impl<L: Dialect, T: CompileTimeValue, V> From<IfFrame<L, T, V>> for ScfFrame<L, T, V> {
    fn from(frame: IfFrame<L, T, V>) -> Self {
        Self::If(frame)
    }
}

impl<L: Dialect, T: CompileTimeValue, V> From<ForFrame<L, T, V>> for ScfFrame<L, T, V> {
    fn from(frame: ForFrame<L, T, V>) -> Self {
        Self::For(frame)
    }
}

impl<L: Dialect, T: CompileTimeValue, V> HasLocation for ScfFrame<L, T, V> {
    fn location(&self) -> Location {
        match self {
            Self::If(frame) => frame.location(),
            Self::For(frame) => frame.location(),
        }
    }
}

impl<I, L, F, C, E, T, V> Frame<I, F, C, E> for ScfFrame<L, T, V>
where
    L: Dialect,
    T: CompileTimeValue,
    IfFrame<L, T, V>: Frame<I, F, C, E>,
    ForFrame<L, T, V>: Frame<I, F, C, E>,
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
pub struct IfFrame<L: Dialect, T: CompileTimeValue, V> {
    pub location: Location,
    pub env: EnvIndex,
    condition: SSAValue,
    then_body: Block,
    else_body: Block,
    results: Vec<ResultValue>,
    phase: IfPhase,
    _marker: IfFrameMarker<L, T, V>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum IfPhase {
    Entry,
    Active,
}

impl<L: Dialect, T: CompileTimeValue, V> IfFrame<L, T, V> {
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

impl<L: Dialect, T: CompileTimeValue, V> HasLocation for IfFrame<L, T, V> {
    fn location(&self) -> Location {
        self.location
    }
}

impl<I, L, F, C, E, T, V> Frame<I, F, C, E> for IfFrame<L, T, V>
where
    I: Env<V, Error = E> + ScfBlockDispatch<L, F, E, V>,
    L: Dialect,
    F: From<IfFrame<L, T, V>>,
    C: ProjectOrSelf<ScfCompletion<V>>,
    E: From<InterpreterError> + From<IndeterminateBranch>,
    T: CompileTimeValue,
    V: BranchCondition + ProductValue,
{
    fn step(self, interp: &mut I) -> Result<FrameEffect<F, C>, E> {
        match self.phase {
            IfPhase::Entry => {
                let block = match interp.read(self.env, self.condition)?.is_truthy() {
                    Some(true) => self.then_body,
                    Some(false) => self.else_body,
                    None => return Err(IndeterminateBranch.into()),
                };
                let child =
                    interp.dispatch_scf_block(self.location, block, self.env, Vec::new())?;
                Ok(FrameEffect::Push {
                    parent: self.active().into(),
                    child,
                })
            }
            IfPhase::Active => Err(InterpreterError::UnexpectedCompletion {
                location: self.location,
                completion: "active scf.if frame stepped",
            }
            .into()),
        }
    }

    fn resume_done(self, _interp: &mut I) -> Result<FrameEffect<F, C>, E> {
        Err(InterpreterError::UnexpectedCompletion {
            location: self.location,
            completion: "scf.if body completed without scf.yield",
        }
        .into())
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
pub struct ForFrame<L: Dialect, T: CompileTimeValue, V> {
    pub location: Location,
    pub env: EnvIndex,
    start: SSAValue,
    end: SSAValue,
    step: SSAValue,
    init_args: Vec<SSAValue>,
    body: Block,
    results: Vec<ResultValue>,
    phase: ForPhase<V>,
    _marker: ForFrameMarker<L, T>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum ForPhase<V> {
    Entry,
    Check { iv: V, end: V, step: V, carried: V },
}

impl<L: Dialect, T: CompileTimeValue, V> ForFrame<L, T, V> {
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

impl<L: Dialect, T: CompileTimeValue, V> HasLocation for ForFrame<L, T, V> {
    fn location(&self) -> Location {
        self.location
    }
}

impl<I, L, F, C, E, T, V> Frame<I, F, C, E> for ForFrame<L, T, V>
where
    I: Env<V, Error = E> + ScfBlockDispatch<L, F, E, V>,
    L: Dialect,
    F: From<ForFrame<L, T, V>>,
    C: ProjectOrSelf<ScfCompletion<V>>,
    E: From<InterpreterError> + From<LoopStepOverflow> + From<IndeterminateBranch>,
    T: CompileTimeValue,
    V: Clone + ForLoopValue + ProductValue,
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

        match phase {
            ForPhase::Entry => {
                let iv = interp.read(env, start)?;
                let end = interp.read(env, end_value)?;
                let step = interp.read(env, step_value)?;
                let carried = V::new_product(interp.read_many(env, init_args.as_slice())?);
                Ok(FrameEffect::Continue(
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
                            iv,
                            end,
                            step,
                            carried,
                        },
                        _marker: PhantomData,
                    }
                    .into(),
                ))
            }
            ForPhase::Check {
                iv,
                end,
                step,
                carried,
            } => {
                if iv.loop_condition(&end) != Some(true) {
                    write_results(interp, env, results.as_slice(), carried)?;
                    return Ok(FrameEffect::Done);
                }

                let args = loop_body_args(iv.clone(), carried.clone(), init_args.len());
                let child = interp.dispatch_scf_block(location, body, env, args)?;
                Ok(FrameEffect::Push {
                    parent: Self {
                        location,
                        env,
                        start,
                        end: end_value,
                        step: step_value,
                        init_args,
                        body,
                        results,
                        phase: ForPhase::Check {
                            iv,
                            end,
                            step,
                            carried,
                        },
                        _marker: PhantomData,
                    }
                    .into(),
                    child,
                })
            }
        }
    }

    fn resume_done(self, _interp: &mut I) -> Result<FrameEffect<F, C>, E> {
        Err(InterpreterError::UnexpectedCompletion {
            location: self.location,
            completion: "scf.for body completed without scf.yield",
        }
        .into())
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
                let next_iv = iv.loop_step(&step).ok_or(LoopStepOverflow)?;
                Ok(FrameEffect::Continue(
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
                    .into(),
                ))
            }
            Err(completion) => Ok(FrameEffect::Complete(completion)),
        }
    }
}

impl<L, I, F, C, E, V, T> Interpretable<L, I, F, C, E, ConcreteTransfer<V>> for If<T>
where
    L: Dialect,
    F: From<IfFrame<L, T, V>>,
    T: CompileTimeValue,
{
    fn interpret(
        &self,
        location: Location,
        env: EnvIndex,
        _interp: &mut I,
    ) -> Result<StatementEffect<F, C, ConcreteTransfer<V>>, E> {
        Ok(StatementEffect::Push(
            IfFrame::<L, T, V>::new(location, env, self).into(),
        ))
    }
}

impl<L, I, F, C, E, V, T> Interpretable<L, I, F, C, E, ConcreteTransfer<V>> for For<T>
where
    L: Dialect,
    F: From<ForFrame<L, T, V>>,
    T: CompileTimeValue,
{
    fn interpret(
        &self,
        location: Location,
        env: EnvIndex,
        _interp: &mut I,
    ) -> Result<StatementEffect<F, C, ConcreteTransfer<V>>, E> {
        Ok(StatementEffect::Push(
            ForFrame::<L, T, V>::new(location, env, self).into(),
        ))
    }
}

impl<L, I, F, C, E, V, T> Interpretable<L, I, F, C, E, ConcreteTransfer<V>> for Yield<T>
where
    L: Dialect,
    I: Env<V, Error = E>,
    C: TryLiftFrom<ScfCompletion<V>>,
    E: From<<C as TryLiftFrom<ScfCompletion<V>>>::Error>,
    T: CompileTimeValue,
    V: ProductValue,
{
    fn interpret(
        &self,
        _location: Location,
        env: EnvIndex,
        interp: &mut I,
    ) -> Result<StatementEffect<F, C, ConcreteTransfer<V>>, E> {
        let values = interp.read_many(env, self.values.as_slice())?;
        Ok(StatementEffect::Complete(C::try_lift_from(
            ScfCompletion::Yield(V::new_product(values)),
        )?))
    }
}

impl<L, I, F, C, E, V, T> Interpretable<L, I, F, C, E, ConcreteTransfer<V>>
    for StructuredControlFlow<T>
where
    L: Dialect,
    I: Env<V, Error = E>,
    F: From<IfFrame<L, T, V>> + From<ForFrame<L, T, V>>,
    C: TryLiftFrom<ScfCompletion<V>>,
    E: From<<C as TryLiftFrom<ScfCompletion<V>>>::Error>,
    T: CompileTimeValue,
    V: ProductValue,
{
    fn interpret(
        &self,
        location: Location,
        env: EnvIndex,
        interp: &mut I,
    ) -> Result<StatementEffect<F, C, ConcreteTransfer<V>>, E> {
        match self {
            StructuredControlFlow::If(op) => {
                <If<T> as Interpretable<L, I, F, C, E, ConcreteTransfer<V>>>::interpret(
                    op, location, env, interp,
                )
            }
            StructuredControlFlow::For(op) => {
                <For<T> as Interpretable<L, I, F, C, E, ConcreteTransfer<V>>>::interpret(
                    op, location, env, interp,
                )
            }
            StructuredControlFlow::Yield(op) => {
                <Yield<T> as Interpretable<L, I, F, C, E, ConcreteTransfer<V>>>::interpret(
                    op, location, env, interp,
                )
            }
        }
    }
}

fn loop_body_args<V: Clone + ProductValue>(iv: V, carried: V, init_arg_count: usize) -> Vec<V> {
    let mut args = Vec::with_capacity(1 + init_arg_count);
    args.push(iv);
    if let Some(product) = carried.as_product() {
        args.extend(product.iter().cloned());
    } else if init_arg_count > 0 {
        args.push(carried);
    }
    args
}

fn write_results<I, V>(
    interp: &mut I,
    env: EnvIndex,
    results: &[ResultValue],
    value: V,
) -> Result<(), I::Error>
where
    I: Env<V>,
    V: ProductValue,
    I::Error: From<InterpreterError>,
{
    let results = results
        .iter()
        .copied()
        .map(SSAValue::from)
        .collect::<Vec<_>>();
    interp.write_product(env, results.as_slice(), value)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IndeterminateBranch;

impl std::fmt::Display for IndeterminateBranch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "indeterminate scf branch condition")
    }
}

impl std::error::Error for IndeterminateBranch {}

impl From<IndeterminateBranch> for InterpreterError {
    fn from(_: IndeterminateBranch) -> Self {
        Self::Custom("indeterminate scf branch condition")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LoopStepOverflow;

impl std::fmt::Display for LoopStepOverflow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "scf.for induction variable overflow")
    }
}

impl std::error::Error for LoopStepOverflow {}

impl From<LoopStepOverflow> for InterpreterError {
    fn from(_: LoopStepOverflow) -> Self {
        Self::Custom("scf.for induction variable overflow")
    }
}
