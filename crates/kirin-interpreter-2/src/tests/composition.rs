use kirin_arith::{ArithType, ArithValue};
use kirin_constant::Constant;
use kirin_ir::{CompileStage, Pipeline, StageInfo, TestSSAValue};

use crate::{
    ConsumeEffect, Interpretable, Interpreter, InterpreterError, Lift, Machine, ProjectMachine,
    ProjectMachineMut,
    control::Shell,
    interpreter::{Position, SingleStage},
};

#[derive(Debug, Clone, PartialEq, Eq, Hash, kirin_ir::Dialect)]
#[kirin(builders, type = ArithType, crate = kirin_ir)]
#[wraps]
enum LiftLanguage {
    Constant(Constant<ArithValue, ArithType>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum LeafEffect {
    Record(ArithValue),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LeafStop {
    Stored,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct LeafMachine {
    seen: Vec<ArithValue>,
}

impl<'ir> Machine<'ir> for LeafMachine {
    type Effect = LeafEffect;
    type Stop = LeafStop;
    type Seed = ();
}

impl<'ir> ConsumeEffect<'ir> for LeafMachine {
    type Error = InterpreterError;

    fn consume_effect(
        &mut self,
        effect: Self::Effect,
    ) -> Result<Shell<Self::Stop, Self::Seed>, Self::Error> {
        match effect {
            LeafEffect::Record(value) => {
                self.seen.push(value);
                Ok(Shell::Stop(LeafStop::Stored))
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum CompositeEffect {
    Leaf(LeafEffect),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CompositeStop {
    Leaf(LeafStop),
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct CompositeMachine {
    leaf: LeafMachine,
    shared_advances: usize,
}

impl<'ir> Machine<'ir> for CompositeMachine {
    type Effect = CompositeEffect;
    type Stop = CompositeStop;
    type Seed = ();
}

impl<'ir> ConsumeEffect<'ir> for CompositeMachine {
    type Error = InterpreterError;

    fn consume_effect(
        &mut self,
        effect: Self::Effect,
    ) -> Result<Shell<Self::Stop, Self::Seed>, Self::Error> {
        match effect {
            CompositeEffect::Leaf(effect) => self
                .leaf
                .consume_effect(effect)
                .map(|control| control.map_stop(CompositeStop::Leaf)),
        }
    }
}

impl ProjectMachine<LeafMachine> for CompositeMachine {
    fn project(&self) -> &LeafMachine {
        &self.leaf
    }
}

impl ProjectMachineMut<LeafMachine> for CompositeMachine {
    fn project_mut(&mut self) -> &mut LeafMachine {
        &mut self.leaf
    }
}

impl Lift<CompositeEffect> for LeafEffect {
    fn lift(self) -> CompositeEffect {
        CompositeEffect::Leaf(self)
    }
}

impl Lift<CompositeStop> for LeafStop {
    fn lift(self) -> CompositeStop {
        CompositeStop::Leaf(self)
    }
}

type LiftInterp<'ir> =
    SingleStage<'ir, LiftLanguage, ArithValue, CompositeMachine, InterpreterError>;

impl<'ir> Interpretable<'ir, LiftInterp<'ir>> for Constant<ArithValue, ArithType> {
    type Effect = LeafEffect;
    type Error = InterpreterError;

    fn interpret(&self, _interp: &mut LiftInterp<'ir>) -> Result<LeafEffect, Self::Error> {
        Ok(LeafEffect::Record(self.value.clone()))
    }
}

impl<'ir> Interpretable<'ir, LiftInterp<'ir>> for LiftLanguage {
    type Effect = CompositeEffect;
    type Error = InterpreterError;

    fn interpret(&self, interp: &mut LiftInterp<'ir>) -> Result<CompositeEffect, Self::Error> {
        match self {
            LiftLanguage::Constant(inner) => {
                let effect = interp.interpret_local(inner)?;
                Ok(effect.lift())
            }
        }
    }
}

fn make_constant(value: i64) -> Constant<ArithValue, ArithType> {
    Constant {
        value: ArithValue::I64(value),
        result: TestSSAValue(0).into(),
        marker: std::marker::PhantomData,
    }
}

#[test]
fn interpret_local_and_lifted_return_different_effect_types() {
    let mut pipeline: Pipeline<StageInfo<LiftLanguage>> = Pipeline::new();
    let stage_id: CompileStage = pipeline.add_stage().stage(StageInfo::default()).new();
    let mut interp = LiftInterp::new(&pipeline, stage_id, CompositeMachine::default());
    let stmt = make_constant(7);

    let local = interp.interpret_local(&stmt).unwrap();
    let lifted = interp.interpret_lifted(&stmt).unwrap();

    assert_eq!(local, LeafEffect::Record(ArithValue::I64(7)));
    assert_eq!(lifted, CompositeEffect::Leaf(local));
}

#[test]
fn project_machine_helpers_reach_leaf_machine() {
    let mut pipeline: Pipeline<StageInfo<LiftLanguage>> = Pipeline::new();
    let stage_id: CompileStage = pipeline.add_stage().stage(StageInfo::default()).new();
    let mut interp = LiftInterp::new(&pipeline, stage_id, CompositeMachine::default());

    interp
        .project_machine_mut::<LeafMachine>()
        .seen
        .push(ArithValue::I64(9));

    assert_eq!(
        interp.project_machine::<LeafMachine>().seen,
        vec![ArithValue::I64(9)]
    );
}

#[test]
fn consume_local_effect_mutates_only_projected_submachine() {
    let mut pipeline: Pipeline<StageInfo<LiftLanguage>> = Pipeline::new();
    let stage_id: CompileStage = pipeline.add_stage().stage(StageInfo::default()).new();
    let mut interp = LiftInterp::new(&pipeline, stage_id, CompositeMachine::default());

    let control = interp
        .consume_local_effect::<LeafMachine>(LeafEffect::Record(ArithValue::I64(3)))
        .unwrap();

    assert_eq!(control, Shell::Stop(LeafStop::Stored));
    assert_eq!(
        interp.project_machine::<LeafMachine>().seen,
        vec![ArithValue::I64(3)]
    );
    assert_eq!(interp.machine().shared_advances, 0);
    assert_eq!(interp.last_stop(), None);
}

#[test]
fn consume_lifted_effect_returns_top_level_control() {
    let mut pipeline: Pipeline<StageInfo<LiftLanguage>> = Pipeline::new();
    let stage_id: CompileStage = pipeline.add_stage().stage(StageInfo::default()).new();
    let mut interp = LiftInterp::new(&pipeline, stage_id, CompositeMachine::default());

    let control = interp
        .consume_lifted_effect(LeafEffect::Record(ArithValue::I64(4)))
        .unwrap();

    assert_eq!(control, Shell::Stop(CompositeStop::Leaf(LeafStop::Stored)));
    assert_eq!(
        interp.project_machine::<LeafMachine>().seen,
        vec![ArithValue::I64(4)]
    );
}

#[test]
fn consume_local_control_lifts_stop_and_applies_shell_mutation() {
    let mut pipeline: Pipeline<StageInfo<LiftLanguage>> = Pipeline::new();
    let stage_id: CompileStage = pipeline.add_stage().stage(StageInfo::default()).new();
    let mut interp = LiftInterp::new(&pipeline, stage_id, CompositeMachine::default());

    interp
        .consume_local_control(Shell::Stop(LeafStop::Stored))
        .unwrap();

    assert_eq!(interp.cursor_depth(), 0);
    assert_eq!(
        interp.last_stop(),
        Some(&CompositeStop::Leaf(LeafStop::Stored))
    );
}
