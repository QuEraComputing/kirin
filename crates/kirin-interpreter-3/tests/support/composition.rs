#![allow(dead_code)]

use std::convert::Infallible;

use kirin_interpreter_3::{
    Effect, InterpError, Interpretable, Lift, Machine, SingleStage, ValueRead,
};
use kirin_ir::{Dialect, Placeholder, Region, ResultValue, SSAValue, Signature};

use super::{TestType, TestValue};

type CompositeInterpreter<'ir> = SingleStage<'ir, CompositeDialect, TestValue, RecordingMachine>;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum RecordingEffect {
    Note(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum InnerRecordingEffect {
    Note(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum RecordingError {
    Boom(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum InnerRecordingError {
    Boom(String),
}

impl Lift<InnerRecordingEffect> for RecordingEffect {
    fn lift(from: InnerRecordingEffect) -> Self {
        match from {
            InnerRecordingEffect::Note(note) => Self::Note(note),
        }
    }
}

impl Lift<InnerRecordingError> for RecordingError {
    fn lift(from: InnerRecordingError) -> Self {
        match from {
            InnerRecordingError::Boom(note) => Self::Boom(note),
        }
    }
}

#[derive(Debug, Default)]
pub struct RecordingMachine {
    pub log: Vec<String>,
}

impl Machine for RecordingMachine {
    type Effect = RecordingEffect;
    type Error = RecordingError;

    fn consume_effect(&mut self, effect: Self::Effect) -> Result<(), Self::Error> {
        match effect {
            RecordingEffect::Note(note) => {
                self.log.push(note);
                Ok(())
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect)]
#[kirin(builders, type = TestType, crate = kirin_ir)]
pub struct CompositeFunctionDef {
    pub body: Region,
    pub sig: Signature<TestType>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect)]
#[kirin(builders, type = TestType, crate = kirin_ir)]
pub struct CompositeConstI64 {
    pub value: i64,
    pub result: ResultValue,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect)]
#[kirin(builders, terminator, type = TestType, crate = kirin_ir)]
pub struct CompositeStop {
    pub value: SSAValue,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect)]
#[kirin(builders, type = TestType, crate = kirin_ir)]
pub struct EmitMachine {
    pub value: i64,
    pub result: ResultValue,
    pub note: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect)]
#[kirin(builders, type = TestType, crate = kirin_ir)]
pub struct LiftedMachine {
    pub value: i64,
    pub result: ResultValue,
    pub note: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect)]
#[kirin(builders, type = TestType, crate = kirin_ir)]
pub struct LiftedError {
    pub note: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect)]
#[kirin(builders, type = TestType, crate = kirin_ir)]
pub enum CompositeDialect {
    #[wraps]
    FunctionDef(CompositeFunctionDef),
    #[wraps]
    ConstI64(CompositeConstI64),
    #[wraps]
    Stop(CompositeStop),
    #[wraps]
    EmitMachine(EmitMachine),
    #[wraps]
    LiftedMachine(LiftedMachine),
    #[wraps]
    LiftedError(LiftedError),
}

impl<'ir> Interpretable<CompositeInterpreter<'ir>> for CompositeConstI64 {
    type Effect = RecordingEffect;
    type Error = RecordingError;

    fn interpret(
        &self,
        _interp: &mut CompositeInterpreter<'ir>,
    ) -> Result<Effect<TestValue, Self::Effect>, InterpError<Self::Error>> {
        Ok(
            Effect::BindValue(self.result.into(), TestValue::from(self.value))
                .then(Effect::Advance),
        )
    }
}

impl<'ir> Interpretable<CompositeInterpreter<'ir>> for CompositeStop {
    type Effect = RecordingEffect;
    type Error = RecordingError;

    fn interpret(
        &self,
        interp: &mut CompositeInterpreter<'ir>,
    ) -> Result<Effect<TestValue, Self::Effect>, InterpError<Self::Error>> {
        Ok(Effect::Stop(interp.read(self.value)?))
    }
}

impl<'ir> Interpretable<CompositeInterpreter<'ir>> for EmitMachine {
    type Effect = RecordingEffect;
    type Error = RecordingError;

    fn interpret(
        &self,
        _interp: &mut CompositeInterpreter<'ir>,
    ) -> Result<Effect<TestValue, Self::Effect>, InterpError<Self::Error>> {
        Ok(
            Effect::BindValue(self.result.into(), TestValue::from(self.value))
                .then(Effect::Machine(RecordingEffect::Note(self.note.clone())))
                .then(Effect::Advance),
        )
    }
}

impl<'ir> Interpretable<CompositeInterpreter<'ir>> for LiftedMachine {
    type Effect = InnerRecordingEffect;
    type Error = RecordingError;

    fn interpret(
        &self,
        _interp: &mut CompositeInterpreter<'ir>,
    ) -> Result<Effect<TestValue, Self::Effect>, InterpError<Self::Error>> {
        Ok(
            Effect::BindValue(self.result.into(), TestValue::from(self.value))
                .then(Effect::Machine(InnerRecordingEffect::Note(
                    self.note.clone(),
                )))
                .then(Effect::Advance),
        )
    }
}

impl<'ir> Interpretable<CompositeInterpreter<'ir>> for LiftedError {
    type Effect = Infallible;
    type Error = InnerRecordingError;

    fn interpret(
        &self,
        _interp: &mut CompositeInterpreter<'ir>,
    ) -> Result<Effect<TestValue, Self::Effect>, InterpError<Self::Error>> {
        Err(InterpError::Dialect(InnerRecordingError::Boom(
            self.note.clone(),
        )))
    }
}

impl<'ir> Interpretable<CompositeInterpreter<'ir>> for CompositeDialect {
    type Effect = RecordingEffect;
    type Error = RecordingError;

    fn interpret(
        &self,
        interp: &mut CompositeInterpreter<'ir>,
    ) -> Result<Effect<TestValue, Self::Effect>, InterpError<Self::Error>> {
        match self {
            Self::FunctionDef(_) => unreachable!("function bodies are not directly stepped"),
            Self::ConstI64(inner) => inner.interpret(interp),
            Self::Stop(inner) => inner.interpret(interp),
            Self::EmitMachine(inner) => inner.interpret(interp),
            Self::LiftedMachine(inner) => Ok(lift_effect(inner.interpret(interp)?)),
            Self::LiftedError(inner) => inner
                .interpret(interp)
                .map(lift_infallible_effect)
                .map_err(lift_error),
        }
    }
}

fn lift_infallible_effect(
    effect: Effect<TestValue, Infallible>,
) -> Effect<TestValue, RecordingEffect> {
    match effect {
        Effect::Advance => Effect::Advance,
        Effect::Stay => Effect::Stay,
        Effect::Jump(block, args) => Effect::Jump(block, args),
        Effect::BindValue(ssa, value) => Effect::BindValue(ssa, value),
        Effect::BindProduct(results, value) => Effect::BindProduct(results, value),
        Effect::Return(value) => Effect::Return(value),
        Effect::Yield(value) => Effect::Yield(value),
        Effect::Stop(value) => Effect::Stop(value),
        Effect::Seq(effects) => Effect::Seq(
            effects
                .into_iter()
                .map(|effect| Box::new(lift_infallible_effect(*effect)))
                .collect(),
        ),
        Effect::Machine(effect) => match effect {},
    }
}

fn lift_effect(
    effect: Effect<TestValue, InnerRecordingEffect>,
) -> Effect<TestValue, RecordingEffect> {
    match effect {
        Effect::Advance => Effect::Advance,
        Effect::Stay => Effect::Stay,
        Effect::Jump(block, args) => Effect::Jump(block, args),
        Effect::BindValue(ssa, value) => Effect::BindValue(ssa, value),
        Effect::BindProduct(results, value) => Effect::BindProduct(results, value),
        Effect::Return(value) => Effect::Return(value),
        Effect::Yield(value) => Effect::Yield(value),
        Effect::Stop(value) => Effect::Stop(value),
        Effect::Seq(effects) => Effect::Seq(
            effects
                .into_iter()
                .map(|effect| Box::new(lift_effect(*effect)))
                .collect(),
        ),
        Effect::Machine(effect) => Effect::Machine(RecordingEffect::lift(effect)),
    }
}

fn lift_error(error: InterpError<InnerRecordingError>) -> InterpError<RecordingError> {
    match error {
        InterpError::Interpreter(error) => InterpError::Interpreter(error),
        InterpError::Dialect(error) => InterpError::Dialect(RecordingError::lift(error)),
    }
}

pub fn build_pure_passthrough_program(
    pipeline: &mut kirin_ir::Pipeline<kirin_ir::StageInfo<CompositeDialect>>,
    stage_id: kirin_ir::CompileStage,
) -> kirin_ir::SpecializedFunction {
    let stage = pipeline.stage_mut(stage_id).unwrap();
    stage.with_builder(|b| {
        let staged = b.staged_function().new().unwrap();
        let value = CompositeConstI64::new(b, 42);
        let stop = CompositeStop::new(b, SSAValue::from(value.result));
        let block = b.block().stmt(value).terminator(stop).new();
        let region = b.region().add_block(block).new();
        let body = CompositeFunctionDef::new(b, region, Signature::new(vec![], TestType::I64, ()));
        b.specialize().staged_func(staged).body(body).new().unwrap()
    })
}

pub fn build_machine_routing_program(
    pipeline: &mut kirin_ir::Pipeline<kirin_ir::StageInfo<CompositeDialect>>,
    stage_id: kirin_ir::CompileStage,
) -> kirin_ir::SpecializedFunction {
    let stage = pipeline.stage_mut(stage_id).unwrap();
    stage.with_builder(|b| {
        let staged = b.staged_function().new().unwrap();
        let value = EmitMachine::new(b, 42, String::from("routed"));
        let stop = CompositeStop::new(b, SSAValue::from(value.result));
        let block = b.block().stmt(value).terminator(stop).new();
        let region = b.region().add_block(block).new();
        let body = CompositeFunctionDef::new(b, region, Signature::new(vec![], TestType::I64, ()));
        b.specialize().staged_func(staged).body(body).new().unwrap()
    })
}

pub fn build_mixed_effect_program(
    pipeline: &mut kirin_ir::Pipeline<kirin_ir::StageInfo<CompositeDialect>>,
    stage_id: kirin_ir::CompileStage,
) -> kirin_ir::SpecializedFunction {
    let stage = pipeline.stage_mut(stage_id).unwrap();
    stage.with_builder(|b| {
        let staged = b.staged_function().new().unwrap();
        let value = LiftedMachine::new(b, 7, String::from("lifted"));
        let stop = CompositeStop::new(b, SSAValue::from(value.result));
        let block = b.block().stmt(value).terminator(stop).new();
        let region = b.region().add_block(block).new();
        let body = CompositeFunctionDef::new(b, region, Signature::new(vec![], TestType::I64, ()));
        b.specialize().staged_func(staged).body(body).new().unwrap()
    })
}

pub fn build_mixed_error_program(
    pipeline: &mut kirin_ir::Pipeline<kirin_ir::StageInfo<CompositeDialect>>,
    stage_id: kirin_ir::CompileStage,
) -> kirin_ir::SpecializedFunction {
    let stage = pipeline.stage_mut(stage_id).unwrap();
    stage.with_builder(|b| {
        let staged = b.staged_function().new().unwrap();
        let fail = LiftedError::new(b, String::from("boom"));
        let block = b.block().stmt(fail).new();
        let region = b.region().add_block(block).new();
        let body = CompositeFunctionDef::new(b, region, Signature::new(vec![], TestType::I64, ()));
        b.specialize().staged_func(staged).body(body).new().unwrap()
    })
}
