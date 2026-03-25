use kirin_arith::{Arith, ArithType, ArithValue};
use kirin_cf::ControlFlow;
use kirin_constant::Constant;
use kirin_function::{FunctionBody, Return};
use kirin_ir::{CompileStage, HasArguments, Pipeline, StageInfo};
use kirin_test_languages::CompositeLanguage;
use kirin_test_utils::ir_fixtures::{build_add_one, build_linear_program, build_select_program};

use crate::{
    ConsumeEffect, Control, FuelControl, Interpretable, InterpreterError, Machine, RunResult,
    SingleStageInterpreter, StepOutcome, SuspendReason, ValueStore,
};

#[derive(Debug, Clone, PartialEq, Eq)]
enum TestEffect {
    Advance,
    Replace(kirin_ir::Block),
    Return(ArithValue),
}

#[derive(Debug, Default)]
struct TestMachine;

impl<'ir> Machine<'ir> for TestMachine {
    type Effect = TestEffect;
    type Stop = ArithValue;
}

impl<'ir> ConsumeEffect<'ir> for TestMachine {
    type Error = InterpreterError;

    fn consume_effect(&mut self, effect: Self::Effect) -> Result<Control<Self::Stop>, Self::Error> {
        Ok(match effect {
            TestEffect::Advance => Control::Advance,
            TestEffect::Replace(block) => Control::Replace(block.into()),
            TestEffect::Return(value) => Control::Stop(value),
        })
    }
}

type TestInterp<'ir> =
    SingleStageInterpreter<'ir, CompositeLanguage, ArithValue, TestMachine, InterpreterError>;

fn unsupported(message: &'static str) -> InterpreterError {
    InterpreterError::custom(std::io::Error::other(message))
}

fn expect_i64(value: ArithValue) -> Result<i64, InterpreterError> {
    match value {
        ArithValue::I64(value) => Ok(value),
        _ => Err(unsupported("unsupported arith value in MVP semantics")),
    }
}

fn is_truthy(value: &ArithValue) -> Result<bool, InterpreterError> {
    Ok(expect_i64(value.clone())? != 0)
}

impl<'ir> Interpretable<'ir, TestInterp<'ir>> for Constant<ArithValue, ArithType> {
    type Machine = TestMachine;
    type Error = InterpreterError;

    fn interpret(&self, interp: &mut TestInterp<'ir>) -> Result<TestEffect, Self::Error> {
        interp.write(self.result, self.value.clone())?;
        Ok(TestEffect::Advance)
    }
}

impl<'ir> Interpretable<'ir, TestInterp<'ir>> for Arith<ArithType> {
    type Machine = TestMachine;
    type Error = InterpreterError;

    fn interpret(&self, interp: &mut TestInterp<'ir>) -> Result<TestEffect, Self::Error> {
        match self {
            Arith::Add {
                lhs, rhs, result, ..
            } => {
                let lhs = expect_i64(interp.read(*lhs)?)?;
                let rhs = expect_i64(interp.read(*rhs)?)?;
                interp.write(*result, ArithValue::I64(lhs + rhs))?;
                Ok(TestEffect::Advance)
            }
            _ => Err(unsupported("unsupported arith op in MVP semantics")),
        }
    }
}

impl<'ir> Interpretable<'ir, TestInterp<'ir>> for ControlFlow<ArithType> {
    type Machine = TestMachine;
    type Error = InterpreterError;

    fn interpret(&self, interp: &mut TestInterp<'ir>) -> Result<TestEffect, Self::Error> {
        match self {
            ControlFlow::Branch { target, args } => {
                let values = args
                    .iter()
                    .map(|value| interp.read(*value))
                    .collect::<Result<Vec<_>, _>>()?;
                let block = target.target();
                interp.bind_block_args(block, &values)?;
                Ok(TestEffect::Replace(block))
            }
            ControlFlow::ConditionalBranch {
                condition,
                true_target,
                true_args,
                false_target,
                false_args,
            } => {
                let cond = interp.read(*condition)?;
                let (target, args) = if is_truthy(&cond)? {
                    (true_target.target(), true_args.as_slice())
                } else {
                    (false_target.target(), false_args.as_slice())
                };
                let values = args
                    .iter()
                    .map(|value| interp.read(*value))
                    .collect::<Result<Vec<_>, _>>()?;
                interp.bind_block_args(target, &values)?;
                Ok(TestEffect::Replace(target))
            }
            _ => Err(unsupported("unsupported control-flow op in MVP semantics")),
        }
    }
}

impl<'ir> Interpretable<'ir, TestInterp<'ir>> for Return<ArithType> {
    type Machine = TestMachine;
    type Error = InterpreterError;

    fn interpret(&self, interp: &mut TestInterp<'ir>) -> Result<TestEffect, Self::Error> {
        let values: Vec<_> = self.arguments().copied().collect();
        match values.as_slice() {
            [value] => Ok(TestEffect::Return(interp.read(*value)?)),
            [] => Err(unsupported(
                "void return is not supported in the MVP semantics",
            )),
            _ => Err(unsupported(
                "multi-value return is not supported in the MVP semantics",
            )),
        }
    }
}

impl<'ir> Interpretable<'ir, TestInterp<'ir>> for FunctionBody<ArithType> {
    type Machine = TestMachine;
    type Error = InterpreterError;

    fn interpret(&self, _interp: &mut TestInterp<'ir>) -> Result<TestEffect, Self::Error> {
        Err(unsupported(
            "function bodies are structural and should not be stepped directly",
        ))
    }
}

impl<'ir> Interpretable<'ir, TestInterp<'ir>> for CompositeLanguage {
    type Machine = TestMachine;
    type Error = InterpreterError;

    fn interpret(&self, interp: &mut TestInterp<'ir>) -> Result<TestEffect, Self::Error> {
        match self {
            CompositeLanguage::Arith(op) => op.interpret(interp),
            CompositeLanguage::ControlFlow(op) => op.interpret(interp),
            CompositeLanguage::Constant(op) => op.interpret(interp),
            CompositeLanguage::FunctionBody(op) => op.interpret(interp),
            CompositeLanguage::Return(op) => op.interpret(interp),
        }
    }
}

#[test]
fn run_linear_program_returns_sum() {
    let mut pipeline: Pipeline<StageInfo<CompositeLanguage>> = Pipeline::new();
    let stage_id: CompileStage = pipeline.add_stage().stage(StageInfo::default()).new();
    let spec_fn = build_linear_program(&mut pipeline, stage_id).0;

    let mut interp = TestInterp::new(&pipeline, stage_id, TestMachine);
    let result = interp.run_specialization(spec_fn, &[]).unwrap();

    assert_eq!(result, RunResult::Stopped(ArithValue::I64(15)));
}

#[test]
fn run_add_one_binds_entry_args() {
    let mut pipeline: Pipeline<StageInfo<CompositeLanguage>> = Pipeline::new();
    let stage_id: CompileStage = pipeline.add_stage().stage(StageInfo::default()).new();
    let spec_fn = build_add_one(&mut pipeline, stage_id);

    let mut interp = TestInterp::new(&pipeline, stage_id, TestMachine);
    let result = interp
        .run_specialization(spec_fn, &[ArithValue::I64(5)])
        .unwrap();

    assert_eq!(result, RunResult::Stopped(ArithValue::I64(6)));
}

#[test]
fn run_select_program_handles_cfg_replace() {
    let mut pipeline: Pipeline<StageInfo<CompositeLanguage>> = Pipeline::new();
    let stage_id: CompileStage = pipeline.add_stage().stage(StageInfo::default()).new();
    let spec_fn = build_select_program(&mut pipeline, stage_id);

    let mut interp = TestInterp::new(&pipeline, stage_id, TestMachine);
    let truthy = interp
        .run_specialization(spec_fn, &[ArithValue::I64(7)])
        .unwrap();
    assert_eq!(truthy, RunResult::Stopped(ArithValue::I64(8)));

    let falsy = interp
        .run_specialization(spec_fn, &[ArithValue::I64(0)])
        .unwrap();
    assert_eq!(falsy, RunResult::Stopped(ArithValue::I64(42)));
}

#[test]
fn step_reports_last_statement_before_completion() {
    let mut pipeline: Pipeline<StageInfo<CompositeLanguage>> = Pipeline::new();
    let stage_id: CompileStage = pipeline.add_stage().stage(StageInfo::default()).new();
    let spec_fn = build_linear_program(&mut pipeline, stage_id).0;

    let mut interp = TestInterp::new(&pipeline, stage_id, TestMachine);
    interp.start_specialization(spec_fn, &[]).unwrap();

    assert!(matches!(interp.step().unwrap(), StepOutcome::Stepped(_)));
    assert!(matches!(interp.step().unwrap(), StepOutcome::Stepped(_)));
    assert!(matches!(interp.step().unwrap(), StepOutcome::Stepped(_)));
    assert!(matches!(interp.step().unwrap(), StepOutcome::Stepped(_)));
    assert!(matches!(interp.step().unwrap(), StepOutcome::Completed));
}

#[test]
fn zero_fuel_suspends_before_first_statement() {
    let mut pipeline: Pipeline<StageInfo<CompositeLanguage>> = Pipeline::new();
    let stage_id: CompileStage = pipeline.add_stage().stage(StageInfo::default()).new();
    let spec_fn = build_linear_program(&mut pipeline, stage_id).0;

    let mut interp = TestInterp::new(&pipeline, stage_id, TestMachine).with_fuel(0);
    interp.start_specialization(spec_fn, &[]).unwrap();

    let outcome = interp.step().unwrap();
    assert!(matches!(
        outcome,
        StepOutcome::Suspended(SuspendReason::FuelExhausted)
    ));
    assert_eq!(interp.fuel(), Some(0));
    assert!(interp.current_statement().is_some());
}

#[test]
fn burning_last_fuel_unit_still_steps_once() {
    let mut pipeline: Pipeline<StageInfo<CompositeLanguage>> = Pipeline::new();
    let stage_id: CompileStage = pipeline.add_stage().stage(StageInfo::default()).new();
    let spec_fn = build_linear_program(&mut pipeline, stage_id).0;

    let mut interp = TestInterp::new(&pipeline, stage_id, TestMachine).with_fuel(1);
    interp.start_specialization(spec_fn, &[]).unwrap();

    assert!(matches!(interp.step().unwrap(), StepOutcome::Stepped(_)));
    assert_eq!(interp.fuel(), Some(0));
    assert!(matches!(
        interp.step().unwrap(),
        StepOutcome::Suspended(SuspendReason::FuelExhausted)
    ));
}

#[test]
fn run_suspends_when_fuel_runs_out() {
    let mut pipeline: Pipeline<StageInfo<CompositeLanguage>> = Pipeline::new();
    let stage_id: CompileStage = pipeline.add_stage().stage(StageInfo::default()).new();
    let spec_fn = build_add_one(&mut pipeline, stage_id);

    let mut interp = TestInterp::new(&pipeline, stage_id, TestMachine).with_fuel(2);
    let result = interp
        .run_specialization(spec_fn, &[ArithValue::I64(5)])
        .unwrap();

    assert_eq!(result, RunResult::Suspended(SuspendReason::FuelExhausted));
}
