use std::convert::Infallible;
use std::fmt::{Display, Formatter};

use kirin::ir::TryLiftFrom;
use kirin::prelude::{Dialect, Function, Pipeline};
use kirin_arith::{Arith, ArithConversionError, ArithType, ArithValue};
use kirin_bitwise::Bitwise;
use kirin_cf::ControlFlow;
use kirin_cmp::Cmp;
use kirin_constant::Constant;
use kirin_function::{Lexical, Lifted};
use kirin_interpreter_new::{
    BlockFrame, CallFrame, ConcreteInterpreter, ConcreteTransfer, EnvIndex, Frame, FrameEffect,
    FunctionBodyEntry, FunctionFrame, HasLocation, Interpretable, InterpreterError, Location,
    ProjectOrSelf, RegionFrame, SpecializedFunctionFrame, StagedFunctionFrame, StandardCompletion,
    StandardFrame, StatementEffect, StatementFrame,
};
use kirin_scf::StructuredControlFlow;
use kirin_scf::interpreter_new::{ForFrame, IfFrame, ScfCompletion, ScfFrame};

use crate::language::{HighLevel, LowLevel};
use crate::stage::Stage;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ToyFrame<L: Dialect, V> {
    Standard(StandardFrame<L, V>),
    Scf(ScfFrame<L, ArithType, V>),
}

impl<L: Dialect, V> From<StandardFrame<L, V>> for ToyFrame<L, V> {
    fn from(frame: StandardFrame<L, V>) -> Self {
        Self::Standard(frame)
    }
}

impl<L: Dialect, V> From<StatementFrame> for ToyFrame<L, V> {
    fn from(frame: StatementFrame) -> Self {
        Self::Standard(frame.into())
    }
}

impl<L: Dialect, V> From<BlockFrame<L, V>> for ToyFrame<L, V> {
    fn from(frame: BlockFrame<L, V>) -> Self {
        Self::Standard(frame.into())
    }
}

impl<L: Dialect, V> From<RegionFrame<L, V>> for ToyFrame<L, V> {
    fn from(frame: RegionFrame<L, V>) -> Self {
        Self::Standard(frame.into())
    }
}

impl<L: Dialect, V> From<CallFrame<L, V>> for ToyFrame<L, V> {
    fn from(frame: CallFrame<L, V>) -> Self {
        Self::Standard(frame.into())
    }
}

impl<L: Dialect, V> From<FunctionFrame<L, V>> for ToyFrame<L, V> {
    fn from(frame: FunctionFrame<L, V>) -> Self {
        Self::Standard(frame.into())
    }
}

impl<L: Dialect, V> From<StagedFunctionFrame<L, V>> for ToyFrame<L, V> {
    fn from(frame: StagedFunctionFrame<L, V>) -> Self {
        Self::Standard(frame.into())
    }
}

impl<L: Dialect, V> From<SpecializedFunctionFrame<L, V>> for ToyFrame<L, V> {
    fn from(frame: SpecializedFunctionFrame<L, V>) -> Self {
        Self::Standard(frame.into())
    }
}

impl<L: Dialect, V> From<ScfFrame<L, ArithType, V>> for ToyFrame<L, V> {
    fn from(frame: ScfFrame<L, ArithType, V>) -> Self {
        Self::Scf(frame)
    }
}

impl<L: Dialect, V> From<IfFrame<L, ArithType, V>> for ToyFrame<L, V> {
    fn from(frame: IfFrame<L, ArithType, V>) -> Self {
        Self::Scf(frame.into())
    }
}

impl<L: Dialect, V> From<ForFrame<L, ArithType, V>> for ToyFrame<L, V> {
    fn from(frame: ForFrame<L, ArithType, V>) -> Self {
        Self::Scf(frame.into())
    }
}

impl<L: Dialect, V> HasLocation for ToyFrame<L, V> {
    fn location(&self) -> Location {
        match self {
            Self::Standard(frame) => frame.location(),
            Self::Scf(frame) => frame.location(),
        }
    }
}

impl<I, L, C, E, V> Frame<I, ToyFrame<L, V>, C, E> for ToyFrame<L, V>
where
    L: Dialect,
    StandardFrame<L, V>: Frame<I, ToyFrame<L, V>, C, E>,
    ScfFrame<L, ArithType, V>: Frame<I, ToyFrame<L, V>, C, E>,
{
    fn step(self, interp: &mut I) -> Result<FrameEffect<ToyFrame<L, V>, C>, E> {
        match self {
            Self::Standard(frame) => frame.step(interp),
            Self::Scf(frame) => frame.step(interp),
        }
    }

    fn resume_done(self, interp: &mut I) -> Result<FrameEffect<ToyFrame<L, V>, C>, E> {
        match self {
            Self::Standard(frame) => frame.resume_done(interp),
            Self::Scf(frame) => frame.resume_done(interp),
        }
    }

    fn resume(self, completion: C, interp: &mut I) -> Result<FrameEffect<ToyFrame<L, V>, C>, E> {
        match self {
            Self::Standard(frame) => frame.resume(completion, interp),
            Self::Scf(frame) => frame.resume(completion, interp),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ToyCompletion<V> {
    Standard(StandardCompletion<V>),
    Scf(ScfCompletion<V>),
}

impl<V> TryLiftFrom<StandardCompletion<V>> for ToyCompletion<V> {
    type Error = Infallible;

    fn try_lift_from(completion: StandardCompletion<V>) -> Result<Self, Self::Error> {
        Ok(Self::Standard(completion))
    }
}

impl<V> TryLiftFrom<ScfCompletion<V>> for ToyCompletion<V> {
    type Error = Infallible;

    fn try_lift_from(completion: ScfCompletion<V>) -> Result<Self, Self::Error> {
        Ok(Self::Scf(completion))
    }
}

impl<V> ProjectOrSelf<StandardCompletion<V>> for ToyCompletion<V> {
    type Error = Infallible;

    fn project_or_self(self) -> Result<StandardCompletion<V>, Self> {
        match self {
            Self::Standard(completion) => Ok(completion),
            other => Err(other),
        }
    }
}

impl<V> ProjectOrSelf<ScfCompletion<V>> for ToyCompletion<V> {
    type Error = Infallible;

    fn project_or_self(self) -> Result<ScfCompletion<V>, Self> {
        match self {
            Self::Scf(completion) => Ok(completion),
            other => Err(other),
        }
    }
}

#[derive(Debug)]
pub enum ToyError {
    Core(InterpreterError),
    ArithConversion(ArithConversionError),
}

impl Display for ToyError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Core(error) => Display::fmt(error, f),
            Self::ArithConversion(error) => Display::fmt(error, f),
        }
    }
}

impl std::error::Error for ToyError {}

impl From<InterpreterError> for ToyError {
    fn from(error: InterpreterError) -> Self {
        Self::Core(error)
    }
}

impl From<ArithConversionError> for ToyError {
    fn from(error: ArithConversionError) -> Self {
        Self::ArithConversion(error)
    }
}

impl From<Infallible> for ToyError {
    fn from(error: Infallible) -> Self {
        match error {}
    }
}

impl From<kirin_arith::interpreter_new::DivisionByZero> for ToyError {
    fn from(error: kirin_arith::interpreter_new::DivisionByZero) -> Self {
        InterpreterError::from(error).into()
    }
}

impl From<kirin_bitwise::interpreter_new::ShiftOverflow> for ToyError {
    fn from(error: kirin_bitwise::interpreter_new::ShiftOverflow) -> Self {
        InterpreterError::from(error).into()
    }
}

impl From<kirin_cf::interpreter_new::IndeterminateBranch> for ToyError {
    fn from(error: kirin_cf::interpreter_new::IndeterminateBranch) -> Self {
        InterpreterError::from(error).into()
    }
}

impl From<kirin_scf::interpreter_new::IndeterminateBranch> for ToyError {
    fn from(error: kirin_scf::interpreter_new::IndeterminateBranch) -> Self {
        InterpreterError::from(error).into()
    }
}

impl From<kirin_scf::interpreter_new::LoopStepOverflow> for ToyError {
    fn from(error: kirin_scf::interpreter_new::LoopStepOverflow) -> Self {
        InterpreterError::from(error).into()
    }
}

impl<I, F, E, V> FunctionBodyEntry<HighLevel, I, F, E, V> for HighLevel
where
    Lexical<ArithType>: FunctionBodyEntry<HighLevel, I, F, E, V>,
    E: From<InterpreterError>,
{
    fn enter_function_body(
        &self,
        location: Location,
        env: EnvIndex,
        interp: &mut I,
        args: Vec<V>,
    ) -> Result<F, E> {
        match self {
            Self::Lexical(op) => op.enter_function_body(location, env, interp, args),
            _ => Err(InterpreterError::Custom("expected high-level function body").into()),
        }
    }
}

impl<I, F, C, E, V> Interpretable<HighLevel, I, F, C, E, ConcreteTransfer<V>> for HighLevel
where
    Lexical<ArithType>: Interpretable<HighLevel, I, F, C, E, ConcreteTransfer<V>>,
    StructuredControlFlow<ArithType>: Interpretable<HighLevel, I, F, C, E, ConcreteTransfer<V>>,
    Constant<ArithValue, ArithType>: Interpretable<HighLevel, I, F, C, E, ConcreteTransfer<V>>,
    Arith<ArithType>: Interpretable<HighLevel, I, F, C, E, ConcreteTransfer<V>>,
    Cmp<ArithType>: Interpretable<HighLevel, I, F, C, E, ConcreteTransfer<V>>,
    Bitwise<ArithType>: Interpretable<HighLevel, I, F, C, E, ConcreteTransfer<V>>,
{
    fn interpret(
        &self,
        location: Location,
        env: EnvIndex,
        interp: &mut I,
    ) -> Result<StatementEffect<F, C, ConcreteTransfer<V>>, E> {
        match self {
            Self::Lexical(op) => <Lexical<ArithType> as Interpretable<
                HighLevel,
                I,
                F,
                C,
                E,
                ConcreteTransfer<V>,
            >>::interpret(op, location, env, interp),
            Self::Structured(op) => <StructuredControlFlow<ArithType> as Interpretable<
                HighLevel,
                I,
                F,
                C,
                E,
                ConcreteTransfer<V>,
            >>::interpret(op, location, env, interp),
            Self::Constant(op) => <Constant<ArithValue, ArithType> as Interpretable<
                HighLevel,
                I,
                F,
                C,
                E,
                ConcreteTransfer<V>,
            >>::interpret(op, location, env, interp),
            Self::Arith(op) => <Arith<ArithType> as Interpretable<
                HighLevel,
                I,
                F,
                C,
                E,
                ConcreteTransfer<V>,
            >>::interpret(op, location, env, interp),
            Self::Cmp(op) => <Cmp<ArithType> as Interpretable<
                HighLevel,
                I,
                F,
                C,
                E,
                ConcreteTransfer<V>,
            >>::interpret(op, location, env, interp),
            Self::Bitwise(op) => <Bitwise<ArithType> as Interpretable<
                HighLevel,
                I,
                F,
                C,
                E,
                ConcreteTransfer<V>,
            >>::interpret(op, location, env, interp),
        }
    }
}

impl<I, F, E, V> FunctionBodyEntry<LowLevel, I, F, E, V> for LowLevel
where
    Lifted<ArithType>: FunctionBodyEntry<LowLevel, I, F, E, V>,
    E: From<InterpreterError>,
{
    fn enter_function_body(
        &self,
        location: Location,
        env: EnvIndex,
        interp: &mut I,
        args: Vec<V>,
    ) -> Result<F, E> {
        match self {
            Self::Lifted(op) => op.enter_function_body(location, env, interp, args),
            _ => Err(InterpreterError::Custom("expected low-level function body").into()),
        }
    }
}

impl<I, F, C, E, V> Interpretable<LowLevel, I, F, C, E, ConcreteTransfer<V>> for LowLevel
where
    Lifted<ArithType>: Interpretable<LowLevel, I, F, C, E, ConcreteTransfer<V>>,
    Constant<ArithValue, ArithType>: Interpretable<LowLevel, I, F, C, E, ConcreteTransfer<V>>,
    Arith<ArithType>: Interpretable<LowLevel, I, F, C, E, ConcreteTransfer<V>>,
    Cmp<ArithType>: Interpretable<LowLevel, I, F, C, E, ConcreteTransfer<V>>,
    Bitwise<ArithType>: Interpretable<LowLevel, I, F, C, E, ConcreteTransfer<V>>,
    ControlFlow<ArithType>: Interpretable<LowLevel, I, F, C, E, ConcreteTransfer<V>>,
{
    fn interpret(
        &self,
        location: Location,
        env: EnvIndex,
        interp: &mut I,
    ) -> Result<StatementEffect<F, C, ConcreteTransfer<V>>, E> {
        match self {
            Self::Lifted(op) => <Lifted<ArithType> as Interpretable<
                LowLevel,
                I,
                F,
                C,
                E,
                ConcreteTransfer<V>,
            >>::interpret(op, location, env, interp),
            Self::Constant(op) => <Constant<ArithValue, ArithType> as Interpretable<
                LowLevel,
                I,
                F,
                C,
                E,
                ConcreteTransfer<V>,
            >>::interpret(op, location, env, interp),
            Self::Arith(op) => <Arith<ArithType> as Interpretable<
                LowLevel,
                I,
                F,
                C,
                E,
                ConcreteTransfer<V>,
            >>::interpret(op, location, env, interp),
            Self::Cmp(op) => <Cmp<ArithType> as Interpretable<
                LowLevel,
                I,
                F,
                C,
                E,
                ConcreteTransfer<V>,
            >>::interpret(op, location, env, interp),
            Self::Bitwise(op) => <Bitwise<ArithType> as Interpretable<
                LowLevel,
                I,
                F,
                C,
                E,
                ConcreteTransfer<V>,
            >>::interpret(op, location, env, interp),
            Self::Cf(op) => <ControlFlow<ArithType> as Interpretable<
                LowLevel,
                I,
                F,
                C,
                E,
                ConcreteTransfer<V>,
            >>::interpret(op, location, env, interp),
        }
    }
}

pub fn run_source_i64(
    pipeline: &Pipeline<Stage>,
    function_name: &str,
    args: &[i64],
) -> Result<i64, ToyError> {
    let stage = pipeline
        .stage_by_name("source")
        .ok_or(InterpreterError::Custom("missing source stage"))?;
    let function = resolve_function(pipeline, function_name)?;
    let mut interp: ConcreteInterpreter<
        '_,
        Stage,
        ToyFrame<HighLevel, i64>,
        ToyCompletion<i64>,
        ToyError,
        i64,
    > = ConcreteInterpreter::new(pipeline);
    interp.push_frame(FunctionFrame::<HighLevel, i64>::new(stage, function, args.to_vec()).into());
    expect_i64_return(interp.run()?)
}

pub fn run_lowered_i64(
    pipeline: &Pipeline<Stage>,
    function_name: &str,
    args: &[i64],
) -> Result<i64, ToyError> {
    let stage = pipeline
        .stage_by_name("lowered")
        .ok_or(InterpreterError::Custom("missing lowered stage"))?;
    let function = resolve_function(pipeline, function_name)?;
    let mut interp: ConcreteInterpreter<
        '_,
        Stage,
        ToyFrame<LowLevel, i64>,
        ToyCompletion<i64>,
        ToyError,
        i64,
    > = ConcreteInterpreter::new(pipeline);
    interp.push_frame(FunctionFrame::<LowLevel, i64>::new(stage, function, args.to_vec()).into());
    expect_i64_return(interp.run()?)
}

fn resolve_function(pipeline: &Pipeline<Stage>, function_name: &str) -> Result<Function, ToyError> {
    let symbol = pipeline
        .lookup_symbol(function_name)
        .ok_or(InterpreterError::Custom("missing function symbol"))?;
    pipeline
        .function_by_name(symbol)
        .ok_or(InterpreterError::Custom("missing function"))
        .map_err(ToyError::from)
}

fn expect_i64_return(completion: ToyCompletion<i64>) -> Result<i64, ToyError> {
    match completion {
        ToyCompletion::Standard(StandardCompletion::FunctionReturned(value)) => Ok(value),
        _ => Err(InterpreterError::Custom("expected function return").into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kirin::prelude::ParsePipelineText;

    fn build_pipeline(src: &str) -> Pipeline<Stage> {
        let mut pipeline = Pipeline::new();
        ParsePipelineText::parse(&mut pipeline, src).expect("parse failed");
        pipeline
    }

    #[test]
    fn runs_source_add() {
        let pipeline = build_pipeline(include_str!("../programs/add.kirin"));
        let result = run_source_i64(&pipeline, "main", &[3, 5]).unwrap();
        assert_eq!(result, 8);
    }

    #[test]
    fn runs_source_branching() {
        let pipeline = build_pipeline(include_str!("../programs/branching.kirin"));
        assert_eq!(run_source_i64(&pipeline, "abs", &[-7]).unwrap(), 7);
        assert_eq!(run_source_i64(&pipeline, "abs", &[7]).unwrap(), 7);
    }

    #[test]
    fn runs_source_recursive_factorial() {
        let pipeline = build_pipeline(include_str!("../programs/factorial.kirin"));
        let result = run_source_i64(&pipeline, "factorial", &[5]).unwrap();
        assert_eq!(result, 120);
    }
}
