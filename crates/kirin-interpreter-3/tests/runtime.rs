mod support;

use kirin_interpreter_3::{Interpreter, SingleStage};
use kirin_ir::{CompileStage, Pipeline, StageInfo};

use support::{TestDialect, TestMachine, TestValue, build_jump_program, build_linear_sum_program};

#[test]
fn single_stage_runs_linear_program_to_completion() {
    let mut pipeline: Pipeline<StageInfo<TestDialect>> = Pipeline::new();
    let stage_id: CompileStage = pipeline.add_stage().stage(StageInfo::default()).new();
    let callee = build_linear_sum_program(&mut pipeline, stage_id);
    let mut interp = SingleStage::new(&pipeline, stage_id, TestMachine);

    interp.start_specialization(callee, &[]).unwrap();
    let value = interp.run().unwrap();

    assert_eq!(value, TestValue::from(42));
}

#[test]
fn single_stage_jumps_and_binds_block_arguments() {
    let mut pipeline: Pipeline<StageInfo<TestDialect>> = Pipeline::new();
    let stage_id: CompileStage = pipeline.add_stage().stage(StageInfo::default()).new();
    let callee = build_jump_program(&mut pipeline, stage_id);
    let mut interp = SingleStage::new(&pipeline, stage_id, TestMachine);

    interp.start_specialization(callee, &[]).unwrap();
    let value = interp.run().unwrap();

    assert_eq!(value, TestValue::from(42));
}
