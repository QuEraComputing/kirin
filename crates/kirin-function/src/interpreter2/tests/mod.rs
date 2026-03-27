mod harness;
mod programs;

use kirin_interpreter_2::{
    ProductValue,
    interpreter::{Position, SingleStage},
    result::Run,
};

use harness::{TestLanguage, TestMachine, TestValue, i64};
use programs::{
    build_direct_call_program, build_multi_result_programs, build_recursive_counter_program,
};

type TestInterp<'ir> =
    SingleStage<'ir, TestLanguage, TestValue, TestMachine, kirin_interpreter_2::InterpreterError>;

#[test]
fn direct_same_stage_call_resumes_in_caller_block() {
    let (pipeline, stage_id, entry) = build_direct_call_program();

    let mut interp = TestInterp::new(&pipeline, stage_id, TestMachine::default());
    let result = interp.run_specialization(entry, &[]).unwrap();

    assert_eq!(result, Run::Stopped(i64(42)));
    assert_eq!(interp.cursor_depth(), 0);
}

#[test]
fn recursive_calls_unwind_function_frames() {
    let (pipeline, stage_id, entry) = build_recursive_counter_program();

    let mut interp = TestInterp::new(&pipeline, stage_id, TestMachine::default());
    let result = interp.run_specialization(entry, &[i64(5)]).unwrap();

    assert_eq!(result, Run::Stopped(i64(5)));
    assert_eq!(interp.cursor_depth(), 0);
}

#[test]
fn multi_result_calls_are_implicitly_unpacked_into_result_slots() {
    let (pipeline, stage_id, pair, caller) = build_multi_result_programs();

    let mut interp = TestInterp::new(&pipeline, stage_id, TestMachine::default());
    let caller_result = interp.run_specialization(caller, &[i64(4)]).unwrap();
    assert_eq!(caller_result, Run::Stopped(i64(7)));
    assert_eq!(interp.cursor_depth(), 0);

    let pair_result = interp.run_specialization(pair, &[i64(4)]).unwrap();
    assert_eq!(
        pair_result,
        Run::Stopped(TestValue::new_product(vec![i64(4), i64(3)]))
    );
    assert_eq!(interp.cursor_depth(), 0);
}
