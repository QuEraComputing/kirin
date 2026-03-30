mod support;

use kirin_interpreter_3::{InterpError, Interpreter, InterpreterError, SingleStage};
use kirin_ir::{Pipeline, StageInfo};

use support::{
    TestDialect, TestMachine, TestValue, build_branch_false_program,
    build_branch_nondeterministic_program, build_branch_true_program, build_for_program,
    build_for_program_missing_yield, build_for_program_overflow, build_if_program_false,
    build_if_program_missing_yield, build_if_program_true,
};

#[test]
fn branch_true_selects_then_block() {
    let mut pipeline: Pipeline<StageInfo<TestDialect>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();
    let callee = build_branch_true_program(&mut pipeline, stage_id);
    let mut interp = SingleStage::new(&pipeline, stage_id, TestMachine);

    interp.start_specialization(callee, &[]).unwrap();
    let value = interp.run().unwrap();

    assert_eq!(value, TestValue::from(11));
}

#[test]
fn branch_false_selects_else_block() {
    let mut pipeline: Pipeline<StageInfo<TestDialect>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();
    let callee = build_branch_false_program(&mut pipeline, stage_id);
    let mut interp = SingleStage::new(&pipeline, stage_id, TestMachine);

    interp.start_specialization(callee, &[]).unwrap();
    let value = interp.run().unwrap();

    assert_eq!(value, TestValue::from(22));
}

#[test]
fn branch_nondeterministic_condition_errors() {
    let mut pipeline: Pipeline<StageInfo<TestDialect>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();
    let callee = build_branch_nondeterministic_program(&mut pipeline, stage_id);
    let mut interp = SingleStage::new(&pipeline, stage_id, TestMachine);

    interp.start_specialization(callee, &[]).unwrap();
    let error = interp.run().unwrap_err();

    assert!(matches!(
        error,
        InterpError::Interpreter(InterpreterError::Unsupported(message)) if message.contains("nondeterministic branch conditions")
    ));
}

#[test]
fn scf_if_success_returns_yielded_value() {
    let mut pipeline: Pipeline<StageInfo<TestDialect>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();
    let callee = build_if_program_true(&mut pipeline, stage_id);
    let mut interp = SingleStage::new(&pipeline, stage_id, TestMachine);

    interp.start_specialization(callee, &[]).unwrap();
    let value = interp.run().unwrap();

    assert_eq!(value, TestValue::from(42));
}

#[test]
fn scf_if_false_branch_is_selected() {
    let mut pipeline: Pipeline<StageInfo<TestDialect>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();
    let callee = build_if_program_false(&mut pipeline, stage_id);
    let mut interp = SingleStage::new(&pipeline, stage_id, TestMachine);

    interp.start_specialization(callee, &[]).unwrap();
    let value = interp.run().unwrap();

    assert_eq!(value, TestValue::from(7));
}

#[test]
fn scf_if_missing_yield_errors() {
    let mut pipeline: Pipeline<StageInfo<TestDialect>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();
    let callee = build_if_program_missing_yield(&mut pipeline, stage_id);
    let mut interp = SingleStage::new(&pipeline, stage_id, TestMachine);

    interp.start_specialization(callee, &[]).unwrap();
    let error = interp.run().unwrap_err();

    assert!(matches!(
        error,
        InterpError::Interpreter(InterpreterError::Unsupported(message)) if message.contains("expected yield from scf.if body")
    ));
}

#[test]
fn scf_for_carries_state_to_completion() {
    let mut pipeline: Pipeline<StageInfo<TestDialect>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();
    let callee = build_for_program(&mut pipeline, stage_id);
    let mut interp = SingleStage::new(&pipeline, stage_id, TestMachine);

    interp.start_specialization(callee, &[]).unwrap();
    let value = interp.run().unwrap();

    assert_eq!(value, TestValue::from(16));
}

#[test]
fn scf_for_missing_yield_errors() {
    let mut pipeline: Pipeline<StageInfo<TestDialect>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();
    let callee = build_for_program_missing_yield(&mut pipeline, stage_id);
    let mut interp = SingleStage::new(&pipeline, stage_id, TestMachine);

    interp.start_specialization(callee, &[]).unwrap();
    let error = interp.run().unwrap_err();

    assert!(matches!(
        error,
        InterpError::Interpreter(InterpreterError::Unsupported(message)) if message.contains("expected yield from scf.for body")
    ));
}

#[test]
fn scf_for_induction_overflow_errors() {
    let mut pipeline: Pipeline<StageInfo<TestDialect>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();
    let callee = build_for_program_overflow(&mut pipeline, stage_id);
    let mut interp = SingleStage::new(&pipeline, stage_id, TestMachine);

    interp.start_specialization(callee, &[]).unwrap();
    let error = interp.run().unwrap_err();

    assert!(matches!(
        error,
        InterpError::Interpreter(InterpreterError::Unsupported(message)) if message.contains("induction variable overflow")
    ));
}
