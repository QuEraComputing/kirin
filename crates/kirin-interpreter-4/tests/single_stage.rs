use kirin_arith::{ArithType, ArithValue};
use kirin_constant::Constant;
use kirin_function::{FunctionBody, Return};
use kirin_interpreter_4::concrete::{Action, SingleStage};
use kirin_interpreter_4::cursor::BlockCursor;
use kirin_interpreter_4::error::InterpreterError;
use kirin_interpreter_4::lift::{ProjectMut, ProjectRef};
use kirin_interpreter_4::traits::{Interpretable, Machine, ValueStore};
use kirin_ir::*;

// ---------------------------------------------------------------------------
// TestValue — minimal value type for the test
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum TestValue {
    I64(i64),
}

// ---------------------------------------------------------------------------
// TestDialect — local dialect to satisfy the orphan rule.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect)]
#[kirin(builders, type = ArithType, crate = kirin_ir)]
#[wraps]
enum TestDialect {
    Constant(Constant<ArithValue, ArithType>),
    FunctionBody(FunctionBody<ArithType>),
    #[kirin(terminator)]
    Return(Return<ArithType>),
}

// ---------------------------------------------------------------------------
// Interpretable impl — returns Action directly
// ---------------------------------------------------------------------------

type TestInterp<'ir> = SingleStage<'ir, TestDialect, TestValue>;
type TestAction = Action<TestValue, (), BlockCursor<TestValue>>;

impl<'ir> Interpretable<TestInterp<'ir>> for TestDialect {
    type Effect = TestAction;
    type Error = InterpreterError;

    fn interpret(&self, interp: &mut TestInterp<'ir>) -> Result<TestAction, InterpreterError> {
        match self {
            TestDialect::Constant(c) => {
                let val = match &c.value {
                    ArithValue::I64(n) => TestValue::I64(*n),
                    other => {
                        return Err(InterpreterError::UnhandledEffect(format!(
                            "unsupported ArithValue variant in test: {other:?}"
                        )));
                    }
                };
                interp.write(c.result, val)?;
                Ok(Action::Advance)
            }
            TestDialect::Return(ret) => {
                let val = ret
                    .arguments()
                    .next()
                    .map(|ssa| interp.read(*ssa))
                    .transpose()?
                    .unwrap_or(TestValue::I64(0));
                Ok(Action::Return(val))
            }
            TestDialect::FunctionBody(_) => Err(InterpreterError::UnhandledEffect(
                "FunctionBody not expected in test execution".to_string(),
            )),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

fn build_program(
    pipeline: &mut Pipeline<StageInfo<TestDialect>>,
    stage_id: CompileStage,
    constant_value: i64,
) -> (SpecializedFunction, Block, ResultValue) {
    pipeline.stage_mut(stage_id).unwrap().with_builder(|b| {
        let sf = b.staged_function().new().unwrap();
        let c0 = Constant::<ArithValue, ArithType>::new(b, ArithValue::I64(constant_value));
        let c0_result = c0.result;
        let ret = Return::<ArithType>::new(b, vec![c0_result.into()]);
        let block = b.block().stmt(c0).terminator(ret).new();
        let region = b.region().add_block(block).new();
        let body = FunctionBody::<ArithType>::new(
            b,
            region,
            Signature::new(vec![], ArithType::default(), ()),
        );
        let spec = b.specialize().staged_func(sf).body(body).new().unwrap();
        (spec, block, c0_result)
    })
}

#[test]
fn test_constant_and_run() {
    let mut pipeline: Pipeline<StageInfo<TestDialect>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();
    let (spec, entry_block, _c0_result) = build_program(&mut pipeline, stage_id, 42);

    let mut interp = TestInterp::new(&pipeline, stage_id, ());
    interp.enter_function(spec, entry_block, &[]).unwrap();
    let result = interp.run().unwrap();

    assert_eq!(result, Some(TestValue::I64(42)));
}

#[test]
fn test_step_by_step() {
    let mut pipeline: Pipeline<StageInfo<TestDialect>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();
    let (spec, entry_block, _c0_result) = build_program(&mut pipeline, stage_id, 99);

    let mut interp = TestInterp::new(&pipeline, stage_id, ());
    interp.enter_function(spec, entry_block, &[]).unwrap();

    // BlockCursor::execute runs entire block in one step (Advance is local).
    // Return is structural — driver pops frame.
    assert!(
        interp.step().unwrap(),
        "first step: block runs, return pops frame"
    );
    assert!(!interp.step().unwrap(), "second step: cursor stack empty");
}

// ---------------------------------------------------------------------------
// CounterMachine — demonstrates dialect machine accessed via mutation
// ---------------------------------------------------------------------------

#[derive(Debug, Default)]
struct CounterMachine {
    count: u32,
}

impl Machine for CounterMachine {
    type Effect = ();
    type Error = InterpreterError;

    fn consume_effect(&mut self, _: ()) -> Result<(), InterpreterError> {
        Ok(())
    }
}

type CounterInterp<'ir> = SingleStage<'ir, TestDialect, TestValue, CounterMachine>;
type CounterAction = Action<TestValue, (), BlockCursor<TestValue>>;

impl<'ir> Interpretable<CounterInterp<'ir>> for TestDialect {
    type Effect = CounterAction;
    type Error = InterpreterError;

    fn interpret(
        &self,
        interp: &mut CounterInterp<'ir>,
    ) -> Result<CounterAction, InterpreterError> {
        interp.machine_mut().count += 1;

        match self {
            TestDialect::Constant(c) => {
                let val = match &c.value {
                    ArithValue::I64(n) => TestValue::I64(*n),
                    other => {
                        return Err(InterpreterError::UnhandledEffect(format!(
                            "unsupported ArithValue variant in test: {other:?}"
                        )));
                    }
                };
                interp.write(c.result, val)?;
                Ok(Action::Advance)
            }
            TestDialect::Return(ret) => {
                let val = ret
                    .arguments()
                    .next()
                    .map(|ssa| interp.read(*ssa))
                    .transpose()?
                    .unwrap_or(TestValue::I64(0));
                Ok(Action::Return(val))
            }
            TestDialect::FunctionBody(_) => Err(InterpreterError::UnhandledEffect(
                "FunctionBody not expected in test execution".to_string(),
            )),
        }
    }
}

#[test]
fn test_counter_machine() {
    let mut pipeline: Pipeline<StageInfo<TestDialect>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();
    let (spec, entry_block, _) = build_program(&mut pipeline, stage_id, 42);

    let mut interp = CounterInterp::new(&pipeline, stage_id, CounterMachine::default());
    interp.enter_function(spec, entry_block, &[]).unwrap();
    let _result = interp.run().unwrap();

    assert_eq!(interp.machine().count, 2);
}

// ---------------------------------------------------------------------------
// CompositeMachine — demonstrates ProjectRef/ProjectMut for sub-machines
// ---------------------------------------------------------------------------

#[derive(Debug, Default)]
struct TraceMachine {
    trace: Vec<&'static str>,
}

/// A composite machine with two sub-machines.
#[derive(Debug, Default)]
struct CompositeMachine {
    counter: CounterMachine,
    trace: TraceMachine,
}

impl Machine for CompositeMachine {
    type Effect = ();
    type Error = InterpreterError;

    fn consume_effect(&mut self, _: ()) -> Result<(), InterpreterError> {
        Ok(())
    }
}

// ProjectRef/ProjectMut: composite → sub-machine
impl ProjectRef<CounterMachine> for CompositeMachine {
    fn project_ref(&self) -> &CounterMachine {
        &self.counter
    }
}
impl ProjectMut<CounterMachine> for CompositeMachine {
    fn project_mut(&mut self) -> &mut CounterMachine {
        &mut self.counter
    }
}
impl ProjectRef<TraceMachine> for CompositeMachine {
    fn project_ref(&self) -> &TraceMachine {
        &self.trace
    }
}
impl ProjectMut<TraceMachine> for CompositeMachine {
    fn project_mut(&mut self) -> &mut TraceMachine {
        &mut self.trace
    }
}

type CompositeInterp<'ir> = SingleStage<'ir, TestDialect, TestValue, CompositeMachine>;
type CompositeAction = Action<TestValue, (), BlockCursor<TestValue>>;

impl<'ir> Interpretable<CompositeInterp<'ir>> for TestDialect {
    type Effect = CompositeAction;
    type Error = InterpreterError;

    fn interpret(
        &self,
        interp: &mut CompositeInterp<'ir>,
    ) -> Result<CompositeAction, InterpreterError> {
        // Project to specific sub-machines instead of accessing the whole composite.
        interp.project_machine_mut::<CounterMachine>().count += 1;

        match self {
            TestDialect::Constant(c) => {
                interp
                    .project_machine_mut::<TraceMachine>()
                    .trace
                    .push("constant");
                let val = match &c.value {
                    ArithValue::I64(n) => TestValue::I64(*n),
                    other => {
                        return Err(InterpreterError::UnhandledEffect(format!(
                            "unsupported ArithValue variant in test: {other:?}"
                        )));
                    }
                };
                interp.write(c.result, val)?;
                Ok(Action::Advance)
            }
            TestDialect::Return(ret) => {
                interp
                    .project_machine_mut::<TraceMachine>()
                    .trace
                    .push("return");
                let val = ret
                    .arguments()
                    .next()
                    .map(|ssa| interp.read(*ssa))
                    .transpose()?
                    .unwrap_or(TestValue::I64(0));
                Ok(Action::Return(val))
            }
            TestDialect::FunctionBody(_) => Err(InterpreterError::UnhandledEffect(
                "FunctionBody not expected in test execution".to_string(),
            )),
        }
    }
}

#[test]
fn test_composite_machine_projection() {
    let mut pipeline: Pipeline<StageInfo<TestDialect>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();
    let (spec, entry_block, _) = build_program(&mut pipeline, stage_id, 42);

    let mut interp = CompositeInterp::new(&pipeline, stage_id, CompositeMachine::default());
    interp.enter_function(spec, entry_block, &[]).unwrap();
    let _result = interp.run().unwrap();

    // Counter tracks total statements
    assert_eq!(interp.project_machine::<CounterMachine>().count, 2);
    // Trace tracks per-operation names
    assert_eq!(
        interp.project_machine::<TraceMachine>().trace,
        vec!["constant", "return"]
    );
}

// ---------------------------------------------------------------------------
// Task 8: Push-effect integration test
// ---------------------------------------------------------------------------

/// A statement that pushes an inline block cursor onto the cursor stack.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect)]
#[kirin(builders, type = ArithType, crate = kirin_ir)]
pub struct ExecBlock {
    target: Block,
}

/// A terminator that yields a value from an inline block.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect)]
#[kirin(terminator, builders, type = ArithType, crate = kirin_ir)]
pub struct TestYield {
    value: SSAValue,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect)]
#[kirin(builders, type = ArithType, crate = kirin_ir)]
#[wraps]
enum PushDialect {
    Constant(Constant<ArithValue, ArithType>),
    FunctionBody(FunctionBody<ArithType>),
    ExecBlock(ExecBlock),
    #[kirin(terminator)]
    TestYield(TestYield),
    #[kirin(terminator)]
    Return(Return<ArithType>),
}

type PushInterp<'ir> = SingleStage<'ir, PushDialect, TestValue>;
type PushAction = Action<TestValue, (), BlockCursor<TestValue>>;

impl<'ir> Interpretable<PushInterp<'ir>> for PushDialect {
    type Effect = PushAction;
    type Error = InterpreterError;

    fn interpret(&self, interp: &mut PushInterp<'ir>) -> Result<PushAction, InterpreterError> {
        match self {
            PushDialect::Constant(c) => {
                let val = match &c.value {
                    ArithValue::I64(n) => TestValue::I64(*n),
                    other => {
                        return Err(InterpreterError::UnhandledEffect(format!(
                            "unsupported ArithValue variant: {other:?}"
                        )));
                    }
                };
                interp.write(c.result, val)?;
                Ok(Action::Advance)
            }
            PushDialect::ExecBlock(eb) => {
                let stage = interp.stage_info();
                let cursor = BlockCursor::new(stage, eb.target, vec![], vec![]);
                Ok(Action::Push(cursor))
            }
            PushDialect::TestYield(ty) => {
                let val = interp.read(ty.value)?;
                Ok(Action::Yield(val))
            }
            PushDialect::Return(ret) => {
                let val = ret
                    .arguments()
                    .next()
                    .map(|ssa| interp.read(*ssa))
                    .transpose()?
                    .unwrap_or(TestValue::I64(0));
                Ok(Action::Return(val))
            }
            PushDialect::FunctionBody(_) => Err(InterpreterError::UnhandledEffect(
                "FunctionBody not expected in test execution".to_string(),
            )),
        }
    }
}

#[test]
fn test_push_inline_block() {
    let mut pipeline: Pipeline<StageInfo<PushDialect>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();

    let (spec, outer_block) = pipeline.stage_mut(stage_id).unwrap().with_builder(|b| {
        let sf = b.staged_function().new().unwrap();

        // Inner block: %0 = constant 77; yield %0
        let c0 = Constant::<ArithValue, ArithType>::new(b, ArithValue::I64(77));
        let c0_result = c0.result;
        let ty = TestYield::new(b, c0_result);
        let inner_block = b.block().stmt(c0).terminator(ty).new();

        // Outer block: exec_block inner_block; return %0
        let eb = ExecBlock::new(b, inner_block);
        let ret = Return::<ArithType>::new(b, vec![c0_result.into()]);
        let outer_block = b.block().stmt(eb).terminator(ret).new();

        let region = b
            .region()
            .add_block(outer_block)
            .add_block(inner_block)
            .new();
        let body = FunctionBody::<ArithType>::new(
            b,
            region,
            Signature::new(vec![], ArithType::default(), ()),
        );
        let spec = b.specialize().staged_func(sf).body(body).new().unwrap();
        (spec, outer_block)
    });

    let mut interp = PushInterp::new(&pipeline, stage_id, ());
    interp.enter_function(spec, outer_block, &[]).unwrap();
    let result = interp.run().unwrap();

    assert_eq!(result, Some(TestValue::I64(77)));
}

// ---------------------------------------------------------------------------
// Task 9: Jump-effect integration test
// ---------------------------------------------------------------------------

/// Unconditional branch — jumps to a target block.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect)]
#[kirin(terminator, builders, type = ArithType, crate = kirin_ir)]
pub struct Br {
    target: Block,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect)]
#[kirin(builders, type = ArithType, crate = kirin_ir)]
#[wraps]
enum JumpDialect {
    Constant(Constant<ArithValue, ArithType>),
    FunctionBody(FunctionBody<ArithType>),
    #[kirin(terminator)]
    Br(Br),
    #[kirin(terminator)]
    Return(Return<ArithType>),
}

type JumpInterp<'ir> = SingleStage<'ir, JumpDialect, TestValue>;
type JumpAction = Action<TestValue, (), BlockCursor<TestValue>>;

impl<'ir> Interpretable<JumpInterp<'ir>> for JumpDialect {
    type Effect = JumpAction;
    type Error = InterpreterError;

    fn interpret(&self, interp: &mut JumpInterp<'ir>) -> Result<JumpAction, InterpreterError> {
        match self {
            JumpDialect::Constant(c) => {
                let val = match &c.value {
                    ArithValue::I64(n) => TestValue::I64(*n),
                    other => {
                        return Err(InterpreterError::UnhandledEffect(format!(
                            "unsupported ArithValue variant: {other:?}"
                        )));
                    }
                };
                interp.write(c.result, val)?;
                Ok(Action::Advance)
            }
            JumpDialect::Br(br) => Ok(Action::Jump(br.target, vec![])),
            JumpDialect::Return(ret) => {
                let val = ret
                    .arguments()
                    .next()
                    .map(|ssa| interp.read(*ssa))
                    .transpose()?
                    .unwrap_or(TestValue::I64(0));
                Ok(Action::Return(val))
            }
            JumpDialect::FunctionBody(_) => Err(InterpreterError::UnhandledEffect(
                "FunctionBody not expected in test execution".to_string(),
            )),
        }
    }
}

#[test]
fn test_jump_multi_block() {
    let mut pipeline: Pipeline<StageInfo<JumpDialect>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();

    let (spec, entry_block) = pipeline.stage_mut(stage_id).unwrap().with_builder(|b| {
        let sf = b.staged_function().new().unwrap();

        // block1: %0 = constant 55; return %0
        let c0 = Constant::<ArithValue, ArithType>::new(b, ArithValue::I64(55));
        let c0_result = c0.result;
        let ret = Return::<ArithType>::new(b, vec![c0_result.into()]);
        let block1 = b.block().stmt(c0).terminator(ret).new();

        // block0 (entry): br block1
        let br = Br::new(b, block1);
        let block0 = b.block().terminator(br).new();

        let region = b.region().add_block(block0).add_block(block1).new();
        let body = FunctionBody::<ArithType>::new(
            b,
            region,
            Signature::new(vec![], ArithType::default(), ()),
        );
        let spec = b.specialize().staged_func(sf).body(body).new().unwrap();
        (spec, block0)
    });

    let mut interp = JumpInterp::new(&pipeline, stage_id, ());
    interp.enter_function(spec, entry_block, &[]).unwrap();
    let result = interp.run().unwrap();

    assert_eq!(result, Some(TestValue::I64(55)));
}
