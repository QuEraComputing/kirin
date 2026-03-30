mod support;

use kirin_interpreter_3::{InterpError, Interpreter, SingleStage};
use kirin_ir::{Pipeline, StageInfo};

use support::{
    CompositeDialect, RecordingError, RecordingMachine, TestValue, build_machine_routing_program,
    build_mixed_effect_program, build_mixed_error_program, build_pure_passthrough_program,
};

#[test]
fn composed_pure_dialect_passthrough_works() {
    let mut pipeline: Pipeline<StageInfo<CompositeDialect>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();
    let callee = build_pure_passthrough_program(&mut pipeline, stage_id);
    let mut interp = SingleStage::new(&pipeline, stage_id, RecordingMachine::default());

    interp.start_specialization(callee, &[]).unwrap();
    let value = interp.run().unwrap();

    assert_eq!(value, TestValue::from(42));
    assert!(interp.machine().log.is_empty());
}

#[test]
fn machine_effect_routing_preserves_sequence_order() {
    let mut pipeline: Pipeline<StageInfo<CompositeDialect>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();
    let callee = build_machine_routing_program(&mut pipeline, stage_id);
    let mut interp = SingleStage::new(&pipeline, stage_id, RecordingMachine::default());

    interp.start_specialization(callee, &[]).unwrap();
    let value = interp.run().unwrap();

    assert_eq!(value, TestValue::from(42));
    assert_eq!(interp.machine().log, vec!["routed".to_owned()]);
}

#[test]
fn composed_mixed_effect_lifts_machine_payloads() {
    let mut pipeline: Pipeline<StageInfo<CompositeDialect>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();
    let callee = build_mixed_effect_program(&mut pipeline, stage_id);
    let mut interp = SingleStage::new(&pipeline, stage_id, RecordingMachine::default());

    interp.start_specialization(callee, &[]).unwrap();
    let value = interp.run().unwrap();

    assert_eq!(value, TestValue::from(7));
    assert_eq!(interp.machine().log, vec!["lifted".to_owned()]);
}

#[test]
fn composed_mixed_error_lifts_dialect_errors() {
    let mut pipeline: Pipeline<StageInfo<CompositeDialect>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();
    let callee = build_mixed_error_program(&mut pipeline, stage_id);
    let mut interp = SingleStage::new(&pipeline, stage_id, RecordingMachine::default());

    interp.start_specialization(callee, &[]).unwrap();
    let error = interp.run().unwrap_err();

    assert!(matches!(
        error,
        InterpError::Dialect(RecordingError::Boom(message)) if message == "boom"
    ));
}
