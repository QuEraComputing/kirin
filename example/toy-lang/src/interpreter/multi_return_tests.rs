//! Multiple-return signatures across every engine.
//!
//! A signature's return list is the same `Product` packet that `ret` produces
//! and a call destructures. The engines never read the signature: arity comes
//! from the `ret` operands and the call's result slots, so the declared
//! `-> i64, i64` and the framework agree without any tuple value or type.

use kirin::prelude::{HasStageInfo, ParseProgramText, Program};
use kirin_constprop::ConstPropValue;
use kirin_demand::Demand;
use kirin_interpreter::SameStageLinker;

use crate::interpreter::{
    ToyInterpreter, analyze_classic_liveness, analyze_constprop, analyze_demand, run_source_i64,
};
use crate::language::HighLevel;
use crate::stage::Stage;

type ConstProp = ConstPropValue<i64>;

const PROGRAM: &str = r#"
stage @source fn @sumdiff(i64, i64) -> i64, i64;
stage @source fn @swap(i64, i64) -> i64, i64;
stage @source fn @pick(i64) -> i64, i64;
stage @source fn @main(i64, i64) -> i64;
stage @source fn @first(i64, i64) -> i64;
stage @source fn @short(i64, i64) -> i64;

specialize @source fn @sumdiff(i64, i64) -> i64, i64 cfg {
  ^entry(%a: i64, %b: i64) {
    %s = add %a, %b -> i64;
    %d = sub %a, %b -> i64;
    ret %s, %d;
  }
}
specialize @source fn @swap(i64, i64) -> i64, i64 cfg {
  ^entry(%a: i64, %b: i64) {
    ret %b, %a;
  }
}
specialize @source fn @pick(i64) -> i64, i64 cfg {
  ^entry(%cond: i64) {
    %one = constant 1 -> i64;
    %two = constant 2 -> i64;
    %x, %y = if %cond then block ^yes() {
      yield %one, %one;
    } else block ^no() {
      yield %one, %two;
    } -> i64, i64;
    ret %x, %y;
  }
}
specialize @source fn @main(i64, i64) -> i64 cfg {
  ^entry(%a: i64, %b: i64) {
    %s, %d = call.named @sumdiff(%a, %b) -> i64, i64;
    %p = mul %s, %d -> i64;
    ret %p;
  }
}
specialize @source fn @first(i64, i64) -> i64 cfg {
  ^entry(%a: i64, %b: i64) {
    %x, %y = call.named @swap(%a, %b) -> i64, i64;
    ret %x;
  }
}
specialize @source fn @short(i64, i64) -> i64 cfg {
  ^entry(%a: i64, %b: i64) {
    %x = call.named @swap(%a, %b) -> i64;
    ret %x;
  }
}
"#;

fn program() -> Program<Stage> {
    let mut program: Program<Stage> = Program::new();
    ParseProgramText::parse(&mut program, PROGRAM).expect("program parses");
    program
}

#[test]
fn signature_declares_one_type_per_returned_value() {
    let program = program();
    let stage_id = program.stage_by_name("source").unwrap();
    let info = <Stage as HasStageInfo<HighLevel>>::try_stage_info(program.stage(stage_id).unwrap())
        .unwrap();
    let arities: Vec<usize> = info
        .staged_function_arena()
        .iter()
        .map(|staged| staged.signature().rets().len())
        .collect();
    assert_eq!(arities, [2, 2, 2, 1, 1, 1]);
}

#[test]
fn concrete_call_destructures_a_two_value_return() {
    let program = program();
    assert_eq!(run_source_i64(&program, "main", &[7, 3]).unwrap(), 40);
}

#[test]
fn concrete_root_returns_the_whole_product() {
    let program = program();
    let product = ToyInterpreter::new(&program)
        .call_by_name("source", "sumdiff", [7, 3])
        .unwrap();
    assert_eq!(product.as_slice(), &[10, 4]);
}

#[test]
fn constprop_propagates_each_returned_slot() {
    let program = program();
    let args = [ConstProp::Const(7), ConstProp::Const(3)];
    assert_eq!(
        analyze_constprop(&program, "source", "main", &args).unwrap(),
        ConstProp::Const(40)
    );
}

#[test]
fn constprop_joins_returns_slot_by_slot() {
    // Both arms yield 1 in slot 0 and disagree in slot 1: the join is per
    // slot, so slot 0 stays constant all the way out of the function.
    let program = program();
    let analysis: crate::interpreter::ToyConstProp<'_, i64, SameStageLinker> =
        crate::interpreter::ToyConstProp::new(&program);
    let analyzed = analysis
        .analyze_by_name("source", "pick", [ConstProp::Top])
        .unwrap();
    let response = analyzed.response(analyzed.root()).unwrap();
    assert_eq!(response.as_slice(), &[ConstProp::Const(1), ConstProp::Top]);
}

#[test]
fn demand_flows_through_one_returned_slot_only() {
    // `first` returns only `%x`, which is `swap`'s slot 0, i.e. `%b`.
    let program = program();
    let analyzed = analyze_demand(&program, "source", "first").unwrap();
    let response = analyzed.response(analyzed.root()).unwrap();
    assert_eq!(response.as_slice(), &[Demand::Dead, Demand::Live]);
}

#[test]
fn classic_liveness_treats_the_call_as_two_definitions() {
    let program = program();
    analyze_classic_liveness(&program, "source", "main").unwrap();
}

#[test]
fn binding_one_result_to_a_two_value_return_is_an_arity_error() {
    // No implicit packing: a two-return callee never becomes one tuple value.
    let program = program();
    assert!(run_source_i64(&program, "short", &[7, 3]).is_err());
    let args = [ConstProp::Const(7), ConstProp::Const(3)];
    assert!(analyze_constprop(&program, "source", "short", &args).is_err());
}
