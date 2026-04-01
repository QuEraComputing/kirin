use kirin_arith::{ArithType, ArithValue};
use kirin_constant::Constant;
use kirin_function::{FunctionBody, Return};
use kirin_interpreter_4::concrete::SingleStage;
use kirin_interpreter_4::effect::CursorEffect;
use kirin_interpreter_4::error::InterpreterError;
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
// Interpretable impl — returns CursorEffect which lifts into Action via Lift
// ---------------------------------------------------------------------------

type TestInterp<'ir> = SingleStage<'ir, TestDialect, TestValue>;

impl<'ir> Interpretable<TestInterp<'ir>> for TestDialect {
    type Effect = CursorEffect<TestValue>;
    type Error = InterpreterError;

    fn interpret(
        &self,
        interp: &mut TestInterp<'ir>,
    ) -> Result<CursorEffect<TestValue>, InterpreterError> {
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
                Ok(CursorEffect::Advance)
            }
            TestDialect::Return(_ret) => Ok(CursorEffect::Advance),
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
    let (spec, entry_block, c0_result) = build_program(&mut pipeline, stage_id, 42);

    let mut interp = TestInterp::new(&pipeline, stage_id, ());
    interp.enter_function(spec, entry_block, &[]).unwrap();
    interp.run().unwrap();

    let result_ssa: SSAValue = c0_result.into();
    assert_eq!(interp.read(result_ssa).unwrap(), TestValue::I64(42));
}

#[test]
fn test_step_by_step() {
    let mut pipeline: Pipeline<StageInfo<TestDialect>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();
    let (spec, entry_block, c0_result) = build_program(&mut pipeline, stage_id, 99);

    let mut interp = TestInterp::new(&pipeline, stage_id, ());
    interp.enter_function(spec, entry_block, &[]).unwrap();

    assert!(interp.step().unwrap(), "first step: constant");
    assert!(interp.step().unwrap(), "second step: return terminator");
    assert!(!interp.step().unwrap(), "third step: exhausted");

    let result_ssa: SSAValue = c0_result.into();
    assert_eq!(interp.read(result_ssa).unwrap(), TestValue::I64(99));
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

impl<'ir> Interpretable<CounterInterp<'ir>> for TestDialect {
    type Effect = CursorEffect<TestValue>;
    type Error = InterpreterError;

    fn interpret(
        &self,
        interp: &mut CounterInterp<'ir>,
    ) -> Result<CursorEffect<TestValue>, InterpreterError> {
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
                Ok(CursorEffect::Advance)
            }
            TestDialect::Return(_ret) => Ok(CursorEffect::Advance),
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
    interp.run().unwrap();

    assert_eq!(interp.machine().count, 2);
}
