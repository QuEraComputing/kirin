use kirin_arith::{Arith, ArithType, ArithValue};
use kirin_cf::ControlFlow;
use kirin_constant::Constant;
use kirin_function::{FunctionBody, Return};
use kirin_ir::{CompileStage, HasArguments, Pipeline, StageInfo, Statement};
use kirin_test_languages::CompositeLanguage;
use kirin_test_utils::ir_fixtures::{build_add_one, build_linear_program, build_select_program};

use crate::{
    BlockSeed, ConsumeEffect, Interpretable, InterpreterError, Machine, ValueStore,
    control::{Breakpoint, Breakpoints, Directive, Fuel, Interrupt, Location},
    interpreter::{Driver, Position, SingleStage, StepResult},
    result::{Run, Step, Suspension},
};

#[derive(Debug, Clone, PartialEq, Eq)]
enum TestEffect {
    Advance,
    Replace(BlockSeed<ArithValue>),
    Return(ArithValue),
}

#[derive(Debug, Default)]
struct TestMachine;

impl<'ir> Machine<'ir> for TestMachine {
    type Effect = TestEffect;
    type Stop = ArithValue;
    type Seed = BlockSeed<ArithValue>;
}

impl<'ir> ConsumeEffect<'ir> for TestMachine {
    type Error = InterpreterError;

    fn consume_effect(
        &mut self,
        effect: Self::Effect,
    ) -> Result<Directive<Self::Stop, Self::Seed>, Self::Error> {
        Ok(match effect {
            TestEffect::Advance => Directive::Advance,
            TestEffect::Replace(seed) => Directive::Replace(seed),
            TestEffect::Return(value) => Directive::Stop(value),
        })
    }
}

type TestInterp<'ir> =
    SingleStage<'ir, CompositeLanguage, ArithValue, TestMachine, InterpreterError>;

fn current_statement_via_position<'ir, I>(interp: &I) -> Option<Statement>
where
    I: Position<'ir>,
{
    interp.current_statement()
}

fn current_location_via_position<'ir, I>(interp: &I) -> Option<Location>
where
    I: Position<'ir>,
{
    interp.current_location()
}

fn step_via_driver<'ir, I>(interp: &mut I) -> StepResult<'ir, I>
where
    I: Driver<'ir>,
    <I::Machine as Machine<'ir>>::Effect: Clone,
    Directive<<I::Machine as Machine<'ir>>::Stop, <I::Machine as Machine<'ir>>::Seed>: Clone,
{
    interp.step()
}

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
    type Effect = TestEffect;
    type Error = InterpreterError;

    fn interpret(&self, interp: &mut TestInterp<'ir>) -> Result<TestEffect, Self::Error> {
        interp.write(self.result, self.value.clone())?;
        Ok(TestEffect::Advance)
    }
}

impl<'ir> Interpretable<'ir, TestInterp<'ir>> for Arith<ArithType> {
    type Effect = TestEffect;
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
    type Effect = TestEffect;
    type Error = InterpreterError;

    fn interpret(&self, interp: &mut TestInterp<'ir>) -> Result<TestEffect, Self::Error> {
        match self {
            ControlFlow::Branch { target, args } => {
                let values = interp.read_many(args)?;
                Ok(TestEffect::Replace(BlockSeed::new(target.target(), values)))
            }
            ControlFlow::ConditionalBranch {
                condition,
                true_target,
                true_args,
                false_target,
                false_args,
            } => {
                let cond = interp.read(*condition)?;
                let (block, args) = if is_truthy(&cond)? {
                    (true_target.target(), true_args.as_slice())
                } else {
                    (false_target.target(), false_args.as_slice())
                };
                let values = interp.read_many(args)?;
                Ok(TestEffect::Replace(BlockSeed::new(block, values)))
            }
            _ => Err(unsupported("unsupported control-flow op in MVP semantics")),
        }
    }
}

impl<'ir> Interpretable<'ir, TestInterp<'ir>> for Return<ArithType> {
    type Effect = TestEffect;
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
    type Effect = TestEffect;
    type Error = InterpreterError;

    fn interpret(&self, _interp: &mut TestInterp<'ir>) -> Result<TestEffect, Self::Error> {
        Err(unsupported(
            "function bodies are structural and should not be stepped directly",
        ))
    }
}

impl<'ir> Interpretable<'ir, TestInterp<'ir>> for CompositeLanguage {
    type Effect = TestEffect;
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

    assert_eq!(result, Run::Stopped(ArithValue::I64(15)));
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

    assert_eq!(result, Run::Stopped(ArithValue::I64(6)));
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
    assert_eq!(truthy, Run::Stopped(ArithValue::I64(8)));

    let falsy = interp
        .run_specialization(spec_fn, &[ArithValue::I64(0)])
        .unwrap();
    assert_eq!(falsy, Run::Stopped(ArithValue::I64(42)));
}

#[test]
fn step_reports_last_statement_before_completion() {
    let mut pipeline: Pipeline<StageInfo<CompositeLanguage>> = Pipeline::new();
    let stage_id: CompileStage = pipeline.add_stage().stage(StageInfo::default()).new();
    let spec_fn = build_linear_program(&mut pipeline, stage_id).0;

    let mut interp = TestInterp::new(&pipeline, stage_id, TestMachine);
    interp.start_specialization(spec_fn, &[]).unwrap();

    assert!(matches!(interp.step().unwrap(), Step::Stepped(_)));
    assert!(matches!(interp.step().unwrap(), Step::Stepped(_)));
    assert!(matches!(interp.step().unwrap(), Step::Stepped(_)));
    assert!(matches!(interp.step().unwrap(), Step::Stepped(_)));
    assert!(matches!(interp.step().unwrap(), Step::Completed));
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
        Step::Suspended(Suspension::FuelExhausted)
    ));
    assert_eq!(interp.fuel(), Some(0));
    assert!(interp.current_statement().is_some());
}

#[test]
fn position_trait_reads_single_stage_location() {
    let mut pipeline: Pipeline<StageInfo<CompositeLanguage>> = Pipeline::new();
    let stage_id: CompileStage = pipeline.add_stage().stage(StageInfo::default()).new();
    let spec_fn = build_linear_program(&mut pipeline, stage_id).0;

    let mut interp = TestInterp::new(&pipeline, stage_id, TestMachine);
    interp.start_specialization(spec_fn, &[]).unwrap();

    let first = current_statement_via_position(&interp).unwrap();

    assert_eq!(
        current_location_via_position(&interp),
        Some(Location::BeforeStatement(first))
    );
}

#[test]
fn driver_trait_steps_single_stage() {
    let mut pipeline: Pipeline<StageInfo<CompositeLanguage>> = Pipeline::new();
    let stage_id: CompileStage = pipeline.add_stage().stage(StageInfo::default()).new();
    let spec_fn = build_linear_program(&mut pipeline, stage_id).0;

    let mut interp = TestInterp::new(&pipeline, stage_id, TestMachine);
    interp.start_specialization(spec_fn, &[]).unwrap();
    let first = current_statement_via_position(&interp).unwrap();

    let outcome = step_via_driver(&mut interp).unwrap();

    assert!(matches!(outcome, Step::Stepped(_)));
    assert_eq!(
        current_location_via_position(&interp),
        Some(Location::AfterStatement(first))
    );
}

#[test]
fn burning_last_fuel_unit_still_steps_once() {
    let mut pipeline: Pipeline<StageInfo<CompositeLanguage>> = Pipeline::new();
    let stage_id: CompileStage = pipeline.add_stage().stage(StageInfo::default()).new();
    let spec_fn = build_linear_program(&mut pipeline, stage_id).0;

    let mut interp = TestInterp::new(&pipeline, stage_id, TestMachine).with_fuel(1);
    interp.start_specialization(spec_fn, &[]).unwrap();

    assert!(matches!(interp.step().unwrap(), Step::Stepped(_)));
    assert_eq!(interp.fuel(), Some(0));
    assert!(matches!(
        interp.step().unwrap(),
        Step::Suspended(Suspension::FuelExhausted)
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

    assert_eq!(result, Run::Suspended(Suspension::FuelExhausted));
}

#[test]
fn breakpoint_before_first_statement_suspends_immediately() {
    let mut pipeline: Pipeline<StageInfo<CompositeLanguage>> = Pipeline::new();
    let stage_id: CompileStage = pipeline.add_stage().stage(StageInfo::default()).new();
    let spec_fn = build_linear_program(&mut pipeline, stage_id).0;

    let mut interp = TestInterp::new(&pipeline, stage_id, TestMachine).with_fuel(1);
    interp.start_specialization(spec_fn, &[]).unwrap();
    let first = interp.current_statement().unwrap();
    let breakpoint = Breakpoint::new(stage_id, Location::BeforeStatement(first));
    assert!(interp.add_breakpoint(breakpoint));

    let outcome = interp.step().unwrap();

    assert_eq!(outcome, Step::Suspended(Suspension::Breakpoint));
    assert_eq!(interp.fuel(), Some(1));
    assert_eq!(interp.current_statement(), Some(first));
}

#[test]
fn breakpoint_wins_over_zero_fuel() {
    let mut pipeline: Pipeline<StageInfo<CompositeLanguage>> = Pipeline::new();
    let stage_id: CompileStage = pipeline.add_stage().stage(StageInfo::default()).new();
    let spec_fn = build_linear_program(&mut pipeline, stage_id).0;

    let mut interp = TestInterp::new(&pipeline, stage_id, TestMachine).with_fuel(0);
    interp.start_specialization(spec_fn, &[]).unwrap();
    let first = interp.current_statement().unwrap();
    interp.add_breakpoint(Breakpoint::new(stage_id, Location::BeforeStatement(first)));

    let outcome = interp.step().unwrap();

    assert_eq!(outcome, Step::Suspended(Suspension::Breakpoint));
    assert_eq!(interp.fuel(), Some(0));
}

#[test]
fn after_statement_breakpoint_suspends_before_next_execution() {
    let mut pipeline: Pipeline<StageInfo<CompositeLanguage>> = Pipeline::new();
    let stage_id: CompileStage = pipeline.add_stage().stage(StageInfo::default()).new();
    let spec_fn = build_linear_program(&mut pipeline, stage_id).0;

    let mut interp = TestInterp::new(&pipeline, stage_id, TestMachine).with_fuel(3);
    interp.start_specialization(spec_fn, &[]).unwrap();
    let first = interp.current_statement().unwrap();

    assert!(matches!(interp.step().unwrap(), Step::Stepped(_)));
    interp.add_breakpoint(Breakpoint::new(stage_id, Location::AfterStatement(first)));

    let outcome = interp.step().unwrap();

    assert_eq!(outcome, Step::Suspended(Suspension::Breakpoint));
    assert_eq!(interp.fuel(), Some(2));
}

#[test]
fn run_until_break_suspends_at_current_breakpoint() {
    let mut pipeline: Pipeline<StageInfo<CompositeLanguage>> = Pipeline::new();
    let stage_id: CompileStage = pipeline.add_stage().stage(StageInfo::default()).new();
    let spec_fn = build_linear_program(&mut pipeline, stage_id).0;

    let mut interp = TestInterp::new(&pipeline, stage_id, TestMachine);
    interp.start_specialization(spec_fn, &[]).unwrap();
    let first = interp.current_statement().unwrap();
    interp.add_breakpoint(Breakpoint::new(stage_id, Location::BeforeStatement(first)));

    let result = interp.run_until_break().unwrap();

    assert_eq!(result, Run::Suspended(Suspension::Breakpoint));
    assert_eq!(interp.current_statement(), Some(first));
}

#[test]
fn host_interrupt_suspends_before_execution() {
    let mut pipeline: Pipeline<StageInfo<CompositeLanguage>> = Pipeline::new();
    let stage_id: CompileStage = pipeline.add_stage().stage(StageInfo::default()).new();
    let spec_fn = build_linear_program(&mut pipeline, stage_id).0;

    let mut interp = TestInterp::new(&pipeline, stage_id, TestMachine).with_fuel(1);
    interp.start_specialization(spec_fn, &[]).unwrap();
    interp.request_interrupt();

    let outcome = interp.step().unwrap();

    assert_eq!(outcome, Step::Suspended(Suspension::HostInterrupt));
    assert_eq!(interp.fuel(), Some(1));
    assert!(interp.current_statement().is_some());
}

#[test]
fn breakpoint_wins_over_host_interrupt() {
    let mut pipeline: Pipeline<StageInfo<CompositeLanguage>> = Pipeline::new();
    let stage_id: CompileStage = pipeline.add_stage().stage(StageInfo::default()).new();
    let spec_fn = build_linear_program(&mut pipeline, stage_id).0;

    let mut interp = TestInterp::new(&pipeline, stage_id, TestMachine);
    interp.start_specialization(spec_fn, &[]).unwrap();
    let first = interp.current_statement().unwrap();
    interp.add_breakpoint(Breakpoint::new(stage_id, Location::BeforeStatement(first)));
    interp.request_interrupt();

    let outcome = interp.step().unwrap();

    assert_eq!(outcome, Step::Suspended(Suspension::Breakpoint));
}

#[test]
fn interrupt_is_level_triggered_until_cleared() {
    let mut pipeline: Pipeline<StageInfo<CompositeLanguage>> = Pipeline::new();
    let stage_id: CompileStage = pipeline.add_stage().stage(StageInfo::default()).new();
    let spec_fn = build_linear_program(&mut pipeline, stage_id).0;

    let mut interp = TestInterp::new(&pipeline, stage_id, TestMachine);
    interp.start_specialization(spec_fn, &[]).unwrap();
    interp.request_interrupt();

    assert_eq!(
        interp.step().unwrap(),
        Step::Suspended(Suspension::HostInterrupt)
    );
    assert_eq!(
        interp.step().unwrap(),
        Step::Suspended(Suspension::HostInterrupt)
    );

    interp.clear_interrupt();

    assert!(matches!(interp.step().unwrap(), Step::Stepped(_)));
}
