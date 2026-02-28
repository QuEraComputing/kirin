use kirin_arith::{ArithType, ArithValue};
use kirin_cf::ControlFlow;
use kirin_constant::Constant;
use kirin_function::FunctionBody;
use kirin_interpreter::{DedupScheduler, Frame, FrameStack, InterpreterError};
use kirin_ir::{CompileStage, Pipeline, ResultValue, SpecializedFunction, StageInfo};
use kirin_test_languages::CompositeLanguage;

fn build_fixture() -> (
    Pipeline<StageInfo<CompositeLanguage>>,
    CompileStage,
    SpecializedFunction,
    kirin_ir::Block,
    ResultValue,
) {
    let mut pipeline: Pipeline<StageInfo<CompositeLanguage>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();
    let stage = pipeline.stage_mut(stage_id).unwrap();

    let sf = stage.staged_function().new().unwrap();
    let c0 = Constant::<ArithValue, ArithType>::new(stage, ArithValue::I64(0));
    let c0_result = c0.result;
    let ret = ControlFlow::<ArithType>::op_return(stage, c0_result);
    let block = stage.block().stmt(c0).terminator(ret).new();
    let region = stage.region().add_block(block).new();
    let body = FunctionBody::<ArithType>::new(stage, region);
    let spec = stage.specialize().f(sf).body(body).new().unwrap();
    (pipeline, stage_id, spec, block, c0_result)
}

#[test]
fn test_dedup_scheduler() {
    let mut scheduler = DedupScheduler::new();
    assert!(scheduler.push_unique(1usize));
    assert!(!scheduler.push_unique(1usize));
    assert!(scheduler.push_unique(2usize));

    let mut popped = Vec::new();
    while let Some(v) = scheduler.pop() {
        popped.push(v);
    }
    assert_eq!(popped, vec![1, 2]);
}

#[test]
fn test_frame_stack_invariants() {
    let (_pipeline, stage_id, callee, _block, result) = build_fixture();
    let mut stack: FrameStack<i32, ()> = FrameStack::new();

    assert_eq!(stack.depth(), 0);
    assert_eq!(stack.active_stage_or(stage_id), stage_id);

    stack
        .push::<InterpreterError>(Frame::new(callee, stage_id, ()))
        .unwrap();
    assert_eq!(stack.depth(), 1);
    assert_eq!(stack.active_stage_or(stage_id), stage_id);

    stack.write::<InterpreterError>(result, 7).unwrap();
    let got: &i32 = stack.read::<InterpreterError>(result.into()).unwrap();
    assert_eq!(*got, 7);

    let popped: Frame<i32, ()> = stack.pop::<InterpreterError>().unwrap();
    assert_eq!(popped.callee(), callee);
    assert_eq!(stack.depth(), 0);
}
