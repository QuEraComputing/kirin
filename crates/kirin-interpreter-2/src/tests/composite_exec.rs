use kirin_arith::{Arith, ArithType, ArithValue};
use kirin_cf::ControlFlow;
use kirin_constant::Constant;
use kirin_function::{FunctionBody, Return};
use kirin_ir::{CompileStage, GetInfo, HasArguments, Pipeline, StageInfo, Statement};
use kirin_test_languages::CompositeLanguage;
use kirin_test_utils::ir_fixtures::{build_add_one, build_linear_program, build_select_program};

use crate::{
    ConsumeEffect, Interpretable, InterpreterError, Machine, ValueStore,
    control::{Breakpoint, Breakpoints, Fuel, Interrupt, Location, Shell},
    interpreter::{BlockBindings, Driver, Position, SingleStage, StepResult, TypedStage},
    result::{Run, Step, Suspension},
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

    fn consume_effect(&mut self, effect: Self::Effect) -> Result<Shell<Self::Stop>, Self::Error> {
        Ok(match effect {
            TestEffect::Advance => Shell::Advance,
            TestEffect::Replace(block) => Shell::Replace(block.into()),
            TestEffect::Return(value) => Shell::Stop(value),
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
    Shell<<I::Machine as Machine<'ir>>::Stop>: Clone,
{
    interp.step()
}

fn bind_block_args_via_block_bindings<'ir, I>(
    interp: &mut I,
    block: kirin_ir::Block,
    args: &[<I as ValueStore>::Value],
) -> Result<(), <I as crate::interpreter::Interpreter<'ir>>::Error>
where
    I: BlockBindings<'ir>,
    <I as ValueStore>::Value: Clone,
    <I as crate::interpreter::Interpreter<'ir>>::Error: From<InterpreterError>,
{
    interp.bind_block_args(block, args)
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
fn block_bindings_trait_binds_block_args_and_computes_resume_seed() {
    let mut pipeline: Pipeline<StageInfo<CompositeLanguage>> = Pipeline::new();
    let stage_id: CompileStage = pipeline.add_stage().stage(StageInfo::default()).new();
    let spec_fn = build_add_one(&mut pipeline, stage_id);

    let mut interp = TestInterp::new(&pipeline, stage_id, TestMachine);
    let entry = interp.entry_block(spec_fn).unwrap();
    interp.push_specialization(spec_fn).unwrap();
    bind_block_args_via_block_bindings(&mut interp, entry, &[ArithValue::I64(9)]).unwrap();

    let first = current_statement_via_position(&interp).unwrap();
    let second = (*first.next(interp.stage_info())).unwrap();
    let expected = crate::BlockSeed::at_statement(entry, second).into();
    let arg0 = entry.expect_info(interp.stage_info()).arguments[0];

    assert_eq!(interp.read(arg0.into()).unwrap(), ArithValue::I64(9));
    assert_eq!(interp.resume_seed_after_current().unwrap(), expected);
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
