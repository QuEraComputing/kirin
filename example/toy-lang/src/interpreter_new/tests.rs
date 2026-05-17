use kirin::prelude::{
    GetInfo, HasStageInfo, ParsePipelineText, Pipeline, Product, Signature, StageInfo,
};
use kirin_arith::ArithType;
use kirin_function::{Call, Function, Return};
use kirin_interpreter_new::{AbstractBlockTransfer, AbstractInterpreter, FunctionInvocation};

use super::*;
use crate::language::{HighLevel, LowLevel};
use crate::stage::Stage;

const ADD_LOWERED: &str = r#"
stage @lowered fn @add(i64, i64) -> i64;

specialize @lowered fn @add(i64, i64) -> i64 {
  ^entry(%a: i64, %b: i64) {
    %result = add %a, %b -> i64;
    ret %result;
  }
}
"#;

const BRANCH_LOWERED: &str = r#"
stage @lowered fn @sign(i64) -> i64;

specialize @lowered fn @sign(i64) -> i64 {
  ^entry(%x: i64) {
    %zero = constant 0 -> i64;
    %is_neg = lt %x, %zero -> i64;
    cond_br %is_neg then=^neg() else=^pos();
  }
  ^neg() {
    %one = constant 1 -> i64;
    ret %one;
  }
  ^pos() {
    %zero2 = constant 0 -> i64;
    ret %zero2;
  }
}
"#;

const SAME_BRANCH_LOWERED: &str = r#"
stage @lowered fn @same(i64) -> i64;
specialize @lowered fn @same(i64) -> i64 {
  ^entry(%x: i64) {
    %zero = constant 0 -> i64;
    %is_neg = lt %x, %zero -> i64;
    cond_br %is_neg then=^lhs() else=^rhs();
  }
  ^lhs() { %one = constant 1 -> i64; ret %one; }
  ^rhs() { %also_one = constant 1 -> i64; ret %also_one; }
}
"#;

const SOURCE_FOR_CARRIED_STABLE: &str = r#"
stage @source fn @stable(i64, i64, i64) -> i64;

specialize @source fn @stable(i64, i64, i64) -> i64 {
  ^entry(%lo: i64, %hi: i64, %s: i64) {
    %init = constant 0 -> i64;
    %sum = for %lo in %lo..%hi step %s iter_args(%init) do ^body(%i: i64, %acc: i64) {
      yield %acc;
    } -> i64;
    ret %sum;
  }
}
"#;

const DIRECTIONAL_LOWERED: &str = r#"
stage @lowered fn @add(i64, i64) -> i64;
stage @lowered fn @mul(i64, i64) -> i64;

specialize @lowered fn @add(i64, i64) -> i64 {
  ^entry(%a: i64, %b: i64) {
    %result = add %a, %b -> i64;
    ret %result;
  }
}

specialize @lowered fn @mul(i64, i64) -> i64 {
  ^entry(%a: i64, %b: i64) {
    %result = mul %a, %b -> i64;
    ret %result;
  }
}
"#;

const CROSS_STAGE_CALLS: &str = r#"
stage @source fn @source_to_lowered_to_source(i64) -> i64;
stage @source fn @low_then_high(i64) -> i64;
stage @source fn @source_abs(i64) -> i64;
stage @lowered fn @low_then_high(i64) -> i64;
stage @lowered fn @source_abs(i64) -> i64;

specialize @source fn @source_to_lowered_to_source(i64) -> i64 {
  ^entry(%x: i64) {
    %result = call.named @low_then_high(%x) -> i64;
    ret %result;
  }
}

specialize @source fn @source_abs(i64) -> i64 {
  ^entry(%x: i64) {
    %zero = constant 0 -> i64;
    %is_neg = lt %x, %zero -> i64;
    %result = if %is_neg then ^then() {
      %negated = neg %x -> i64;
      yield %negated;
    } else ^else() {
      yield %x;
    } -> i64;
    ret %result;
  }
}

specialize @lowered fn @low_then_high(i64) -> i64 {
  ^entry(%x: i64) {
    %abs = call.named @source_abs(%x) -> i64;
    %one = constant 1 -> i64;
    %result = add %abs, %one -> i64;
    ret %result;
  }
}
"#;

const CROSS_STAGE_SPECIALIZED_CALLS: &str = r#"
stage @source fn @source_direct_specialized(i64) -> i64;
stage @source fn @dual_impl(i64) -> i64;
stage @lowered fn @dual_impl(i64) -> i64;

specialize @source fn @dual_impl(i64) -> i64 {
  ^entry(%x: i64) {
    %one = constant 1 -> i64;
    %result = add %x, %one -> i64;
    ret %result;
  }
}

specialize @lowered fn @dual_impl(i64) -> i64 {
  ^entry(%x: i64) {
    %hundred = constant 100 -> i64;
    %result = add %x, %hundred -> i64;
    ret %result;
  }
}
"#;

fn build_pipeline(src: &str) -> Pipeline<Stage> {
    let mut pipeline = Pipeline::new();
    ParsePipelineText::parse(&mut pipeline, src).expect("parse failed");
    pipeline
}

fn build_cross_stage_specialized_pipeline() -> Pipeline<Stage> {
    let mut pipeline = build_pipeline(CROSS_STAGE_SPECIALIZED_CALLS);
    let source_stage = pipeline.stage_by_name("source").unwrap();
    let lowered_stage = pipeline.stage_by_name("lowered").unwrap();
    let caller = pipeline
        .resolve_staged_function("source_direct_specialized", source_stage)
        .unwrap();
    let lowered_dual_impl = pipeline
        .resolve_staged_function("dual_impl", lowered_stage)
        .unwrap();
    let lowered_stage_meta = pipeline.stage(lowered_stage).unwrap();
    let lowered_info: &StageInfo<LowLevel> = lowered_stage_meta.try_stage_info().unwrap();
    let lowered_specialized = lowered_dual_impl
        .get_info(lowered_info)
        .unwrap()
        .unique_live_specialization()
        .unwrap();

    let Stage::Source(source_info) = pipeline.stage_mut(source_stage).unwrap() else {
        unreachable!("source stage id resolved to a non-source stage");
    };
    source_info.with_builder(|builder| {
        let x = builder.block_argument().index(0).into();
        let call = Call::<ArithType>::build(builder)
            .in_stage(lowered_stage)
            .specialized(lowered_specialized)
            .args(vec![x])
            .results(1)
            .insert();
        let ret = Return::<ArithType>::new(builder, vec![call.results[0].into()]);
        let block = builder
            .block()
            .argument(ArithType::I64)
            .stmt(call)
            .terminator(ret)
            .new();
        let region = builder.region().add_block(block).new();
        let body = Function::<ArithType>::new(
            builder,
            region,
            Signature::new(vec![ArithType::I64], ArithType::I64, ()),
        );
        builder
            .specialize()
            .staged_func(caller)
            .body(body)
            .new()
            .unwrap();
    });

    pipeline
}

#[test]
fn runs_source_add() {
    let pipeline = build_pipeline(include_str!("../../programs/add.kirin"));
    let result = run_source_i64(&pipeline, "main", &[3, 5]).unwrap();
    assert_eq!(result, 8);
}

#[test]
fn runs_source_branching() {
    let pipeline = build_pipeline(include_str!("../../programs/branching.kirin"));
    assert_eq!(run_source_i64(&pipeline, "abs", &[-7]).unwrap(), 7);
    assert_eq!(run_source_i64(&pipeline, "abs", &[7]).unwrap(), 7);
}

#[test]
fn runs_source_recursive_factorial() {
    let pipeline = build_pipeline(include_str!("../../programs/factorial.kirin"));
    let result = run_source_i64(&pipeline, "factorial", &[5]).unwrap();
    assert_eq!(result, 120);
}

#[test]
fn constprop_source_add() {
    let pipeline = build_pipeline(include_str!("../../programs/add.kirin"));
    let result = analyze_source_constprop(
        &pipeline,
        "main",
        &[ConstProp::Const(3), ConstProp::Const(5)],
    )
    .unwrap();
    assert_eq!(result, ConstProp::Const(8));
}

#[test]
fn constprop_fixpoint_source_add() {
    let pipeline = build_pipeline(include_str!("../../programs/add.kirin"));
    let result = analyze_source_constprop_fixpoint(
        &pipeline,
        "main",
        &[ConstProp::Const(3), ConstProp::Const(5)],
    )
    .unwrap();
    assert_eq!(result, ConstProp::Const(8));
}

#[test]
fn constprop_fixpoint_source_entry_variants() {
    let pipeline = build_pipeline(include_str!("../../programs/add.kirin"));
    let stage = pipeline.stage_by_name("source").unwrap();
    let symbol = pipeline.lookup_symbol("main").unwrap();
    let function = pipeline.function_by_name(symbol).unwrap();
    let staged = pipeline.resolve_staged_function("main", stage).unwrap();
    let stage_meta = pipeline.stage(stage).unwrap();
    let stage_info: &StageInfo<HighLevel> = stage_meta.try_stage_info().unwrap();
    let specialized = staged
        .get_info(stage_info)
        .unwrap()
        .specializations()
        .iter()
        .find(|specialization| !specialization.is_invalidated())
        .unwrap()
        .id();
    let args = || Product::from_vec(vec![ConstProp::Const(3), ConstProp::Const(5)]);

    assert_eq!(
        super::fixpoint::analyze_source_constprop_invocation(
            &pipeline,
            FunctionInvocation::function(stage, function, args())
        )
        .unwrap(),
        ConstProp::Const(8)
    );
    assert_eq!(
        super::fixpoint::analyze_source_constprop_invocation(
            &pipeline,
            FunctionInvocation::staged_function(stage, staged, args())
        )
        .unwrap(),
        ConstProp::Const(8)
    );
    assert_eq!(
        super::fixpoint::analyze_source_constprop_invocation(
            &pipeline,
            FunctionInvocation::specialized_function(stage, specialized, args())
        )
        .unwrap(),
        ConstProp::Const(8)
    );

    let mut staged_interp: AbstractInterpreter<
        '_,
        Stage,
        ToyFrame<HighLevel, ConstProp, AbstractBlockTransfer<ConstProp>>,
        ToyCompletion<ConstProp>,
        ToyError,
        ConstProp,
    > = AbstractInterpreter::new(&pipeline);
    assert_eq!(
        super::run::expect_function_return(
            staged_interp
                .invoke(stage)
                .staged(staged)
                .args(args())
                .unwrap()
        )
        .unwrap(),
        ConstProp::Const(8)
    );

    let mut specialized_interp: AbstractInterpreter<
        '_,
        Stage,
        ToyFrame<HighLevel, ConstProp, AbstractBlockTransfer<ConstProp>>,
        ToyCompletion<ConstProp>,
        ToyError,
        ConstProp,
    > = AbstractInterpreter::new(&pipeline);
    assert_eq!(
        super::run::expect_function_return(
            specialized_interp
                .invoke(stage)
                .specialized(specialized)
                .args(args())
                .unwrap()
        )
        .unwrap(),
        ConstProp::Const(8)
    );
}

#[test]
fn constprop_fixpoint_source_unknown_branch_joins_yields() {
    let pipeline = build_pipeline(include_str!("../../programs/branching.kirin"));
    let result = analyze_source_constprop_fixpoint(&pipeline, "abs", &[ConstProp::Top]).unwrap();
    assert_eq!(result, ConstProp::Top);
}

#[test]
fn constprop_fixpoint_source_for_owner_keeps_stable_carried_value() {
    let pipeline = build_pipeline(SOURCE_FOR_CARRIED_STABLE);
    let result = analyze_source_constprop_fixpoint(
        &pipeline,
        "stable",
        &[ConstProp::Const(0), ConstProp::Top, ConstProp::Const(1)],
    )
    .unwrap();
    assert_eq!(result, ConstProp::Const(0));
}

#[test]
fn constprop_fixpoint_cross_stage_calls_between_source_and_lowered() {
    let pipeline = build_pipeline(CROSS_STAGE_CALLS);

    let source_result = analyze_source_constprop_fixpoint(
        &pipeline,
        "source_to_lowered_to_source",
        &[ConstProp::Const(-7)],
    )
    .unwrap();
    assert_eq!(source_result, ConstProp::Const(8));

    let lowered_result =
        analyze_lowered_constprop_fixpoint(&pipeline, "low_then_high", &[ConstProp::Const(-4)])
            .unwrap();
    assert_eq!(lowered_result, ConstProp::Const(5));
}

#[test]
fn constprop_fixpoint_cross_stage_call_specialized_uses_direct_target() {
    let pipeline = build_cross_stage_specialized_pipeline();

    let result = analyze_source_constprop_fixpoint(
        &pipeline,
        "source_direct_specialized",
        &[ConstProp::Const(5)],
    )
    .unwrap();

    assert_eq!(result, ConstProp::Const(105));
}

#[test]
fn constprop_source_add_with_unknown() {
    let pipeline = build_pipeline(include_str!("../../programs/add.kirin"));
    let result =
        analyze_source_constprop(&pipeline, "main", &[ConstProp::Top, ConstProp::Const(5)])
            .unwrap();
    assert_eq!(result, ConstProp::Top);
}

#[test]
fn constprop_source_known_branch() {
    let pipeline = build_pipeline(include_str!("../../programs/branching.kirin"));
    assert_eq!(
        analyze_source_constprop(&pipeline, "abs", &[ConstProp::Const(-7)]).unwrap(),
        ConstProp::Const(7)
    );
    assert_eq!(
        analyze_source_constprop(&pipeline, "abs", &[ConstProp::Const(7)]).unwrap(),
        ConstProp::Const(7)
    );
}

#[test]
fn constprop_source_unknown_branch_joins_yields() {
    let pipeline = build_pipeline(include_str!("../../programs/branching.kirin"));
    let result = analyze_source_constprop(&pipeline, "abs", &[ConstProp::Top]).unwrap();
    assert_eq!(result, ConstProp::Top);
}

#[test]
fn constprop_lowered_add() {
    let pipeline = build_pipeline(ADD_LOWERED);
    let result = analyze_lowered_constprop(
        &pipeline,
        "add",
        &[ConstProp::Const(2), ConstProp::Const(3)],
    )
    .unwrap();
    assert_eq!(result, ConstProp::Const(5));
}

#[test]
fn constprop_fixpoint_lowered_add() {
    let pipeline = build_pipeline(ADD_LOWERED);
    let result = analyze_lowered_constprop_fixpoint(
        &pipeline,
        "add",
        &[ConstProp::Const(2), ConstProp::Const(3)],
    )
    .unwrap();
    assert_eq!(result, ConstProp::Const(5));
}

#[test]
fn constprop_fixpoint_lowered_unknown_cf_branch_returns_top() {
    let pipeline = build_pipeline(BRANCH_LOWERED);
    let result = analyze_lowered_constprop_fixpoint(&pipeline, "sign", &[ConstProp::Top]).unwrap();
    assert_eq!(result, ConstProp::Top);
}

#[test]
fn constprop_fixpoint_lowered_unknown_cf_branch_joins_matching_returns() {
    let pipeline = build_pipeline(SAME_BRANCH_LOWERED);
    let result = analyze_lowered_constprop_fixpoint(&pipeline, "same", &[ConstProp::Top]).unwrap();
    assert_eq!(result, ConstProp::Const(1));
}

#[test]
fn constprop_fixpoint_forward_dependencies_run_real_toy_functions() {
    let pipeline = build_pipeline(DIRECTIONAL_LOWERED);
    let result = analyze_lowered_constprop_forward_dependencies(
        &pipeline,
        "add",
        &[ConstProp::Const(2), ConstProp::Const(3)],
        "mul",
        &[ConstProp::Const(4), ConstProp::Const(5)],
    )
    .unwrap();

    assert_eq!(result.source_value, ConstProp::Const(5));
    assert_eq!(result.target_value, ConstProp::Const(20));
    assert_eq!(result.source_visits, 1);
    assert_eq!(result.target_visits, 1);
}

#[test]
fn constprop_fixpoint_backward_dependencies_run_real_toy_functions() {
    let pipeline = build_pipeline(DIRECTIONAL_LOWERED);
    let result = analyze_lowered_constprop_backward_dependencies(
        &pipeline,
        "add",
        &[ConstProp::Const(2), ConstProp::Const(3)],
        "mul",
        &[ConstProp::Const(4), ConstProp::Const(5)],
    )
    .unwrap();

    assert_eq!(result.source_value, ConstProp::Const(5));
    assert_eq!(result.target_value, ConstProp::Const(20));
    assert_eq!(result.source_visits, 1);
    assert_eq!(result.target_visits, 1);
}

#[test]
fn constprop_lowered_known_cf_branch() {
    let pipeline = build_pipeline(BRANCH_LOWERED);
    assert_eq!(
        analyze_lowered_constprop(&pipeline, "sign", &[ConstProp::Const(-3)]).unwrap(),
        ConstProp::Const(1)
    );
    assert_eq!(
        analyze_lowered_constprop(&pipeline, "sign", &[ConstProp::Const(5)]).unwrap(),
        ConstProp::Const(0)
    );
}

#[test]
fn constprop_lowered_unknown_cf_branch_returns_top() {
    let pipeline = build_pipeline(BRANCH_LOWERED);
    let result = analyze_lowered_constprop(&pipeline, "sign", &[ConstProp::Top]).unwrap();
    assert_eq!(result, ConstProp::Top);
}

#[test]
fn constprop_lowered_unknown_cf_branch_joins_matching_returns() {
    let pipeline = build_pipeline(SAME_BRANCH_LOWERED);
    let result = analyze_lowered_constprop(&pipeline, "same", &[ConstProp::Top]).unwrap();
    assert_eq!(result, ConstProp::Const(1));
}
