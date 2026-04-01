use kirin_arith::{ArithType, ArithValue};
use kirin_constant::Constant;
use kirin_function::{FunctionBody, Return};
use kirin_interpreter_4::concrete::SingleStage;
use kirin_interpreter_4::effect::CursorEffect;
use kirin_interpreter_4::error::InterpreterError;
use kirin_interpreter_4::traits::{Interpretable, ValueStore};
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
//
// Uses #[derive(Dialect)] with #[wraps] so that all Dialect supertraits
// (IsTerminator, HasResults, HasArguments, IsEdge, HasUngraphs, ...) are
// generated automatically via delegation to the inner types.
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
// Interpretable impl for TestDialect on SingleStage
// ---------------------------------------------------------------------------

impl<'ir> Interpretable<SingleStage<'ir, TestDialect, TestValue>> for TestDialect {
    type Effect = CursorEffect<TestValue>;
    type Error = InterpreterError;

    fn interpret(
        &self,
        interp: &mut SingleStage<'ir, TestDialect, TestValue>,
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
            TestDialect::Return(_ret) => {
                // Return is a terminator; advancing past it exhausts the block cursor.
                Ok(CursorEffect::Advance)
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

#[test]
fn test_constant_and_run() {
    // Build a pipeline with one stage containing a function: %0 = constant 42; ret %0
    let mut pipeline: Pipeline<StageInfo<TestDialect>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();

    let (spec, entry_block, c0_result) = pipeline.stage_mut(stage_id).unwrap().with_builder(|b| {
        let sf = b.staged_function().new().unwrap();
        let c0 = Constant::<ArithValue, ArithType>::new(b, ArithValue::I64(42));
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
    });

    // Create interpreter, enter the function, and run
    let mut interp = SingleStage::<TestDialect, TestValue>::new(&pipeline, stage_id);
    interp
        .enter_function(spec, entry_block, &[])
        .expect("enter_function should succeed");
    interp.run().expect("run should succeed");

    // Verify the constant was written
    let result_ssa: SSAValue = c0_result.into();
    let value = interp.read(result_ssa).expect("result should be bound");
    assert_eq!(value, TestValue::I64(42));
}

#[test]
fn test_step_by_step() {
    // Same program, but drive it with step() to verify step-level control.
    let mut pipeline: Pipeline<StageInfo<TestDialect>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();

    let (spec, entry_block, c0_result) = pipeline.stage_mut(stage_id).unwrap().with_builder(|b| {
        let sf = b.staged_function().new().unwrap();
        let c0 = Constant::<ArithValue, ArithType>::new(b, ArithValue::I64(99));
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
    });

    let mut interp = SingleStage::<TestDialect, TestValue>::new(&pipeline, stage_id);
    interp
        .enter_function(spec, entry_block, &[])
        .expect("enter_function should succeed");

    // Step 1: execute the Constant statement
    let ran = interp.step().expect("step should succeed");
    assert!(ran, "first step should execute the constant");

    // Step 2: execute the Return terminator
    let ran = interp.step().expect("step should succeed");
    assert!(ran, "second step should execute the return terminator");

    // Step 3: block exhausted, no more statements
    let ran = interp.step().expect("step should succeed");
    assert!(!ran, "third step should return false (block exhausted)");

    // Verify the value
    let result_ssa: SSAValue = c0_result.into();
    let value = interp.read(result_ssa).expect("result should be bound");
    assert_eq!(value, TestValue::I64(99));
}
