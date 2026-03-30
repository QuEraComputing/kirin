#![allow(dead_code)]

use kirin_ir::{CompileStage, Pipeline, SSAValue, Signature, StageInfo, Successor};

use super::{
    AddI64, BranchSelect, ConstI64, ForOp, FunctionDef, IfOp, JumpTo, PackTuple, ReturnOp, StopOp,
    TestDialect, TestType, UnknownValue, YieldOp,
};

pub fn build_linear_sum_program(
    pipeline: &mut Pipeline<StageInfo<TestDialect>>,
    stage_id: CompileStage,
) -> kirin_ir::SpecializedFunction {
    let stage = pipeline.stage_mut(stage_id).unwrap();
    stage.with_builder(|b| {
        let staged = b.staged_function().new().unwrap();

        let c2 = ConstI64::new(b, 2);
        let c40 = ConstI64::new(b, 40);
        let add = AddI64::new(b, SSAValue::from(c2.result), SSAValue::from(c40.result));
        let stop = StopOp::new(b, SSAValue::from(add.result));
        let block = b
            .block()
            .stmt(c2)
            .stmt(c40)
            .stmt(add)
            .terminator(stop)
            .new();
        let region = b.region().add_block(block).new();
        let body = FunctionDef::new(b, region, Signature::new(vec![], TestType::I64, ()));
        b.specialize().staged_func(staged).body(body).new().unwrap()
    })
}

pub fn build_jump_program(
    pipeline: &mut Pipeline<StageInfo<TestDialect>>,
    stage_id: CompileStage,
) -> kirin_ir::SpecializedFunction {
    let stage = pipeline.stage_mut(stage_id).unwrap();
    stage.with_builder(|b| {
        let staged = b.staged_function().new().unwrap();

        let exit = b.block().argument(TestType::I64).new();
        let exit_arg: SSAValue = b.block_arena()[exit].arguments[0].into();
        let exit_stop = StopOp::new(b, exit_arg);
        {
            let exit_info = b.block_arena_mut().get_mut(exit).unwrap();
            exit_info.terminator = Some(exit_stop.into());
        }

        let c42 = ConstI64::new(b, 42);
        let jump = JumpTo::new(
            b,
            Successor::from_block(exit),
            vec![SSAValue::from(c42.result)],
        );
        let entry = b.block().stmt(c42).terminator(jump).new();
        let region = b.region().add_block(entry).add_block(exit).new();
        let body = FunctionDef::new(b, region, Signature::new(vec![], TestType::I64, ()));
        b.specialize().staged_func(staged).body(body).new().unwrap()
    })
}

pub fn build_region_yield_program(
    pipeline: &mut Pipeline<StageInfo<TestDialect>>,
    stage_id: CompileStage,
) -> kirin_ir::SpecializedFunction {
    let stage = pipeline.stage_mut(stage_id).unwrap();
    stage.with_builder(|b| {
        let staged = b.staged_function().new().unwrap();

        let exit = b.block().argument(TestType::I64).new();
        let exit_arg: SSAValue = b.block_arena()[exit].arguments[0].into();
        let exit_yield = YieldOp::new(b, exit_arg);
        {
            let exit_info = b.block_arena_mut().get_mut(exit).unwrap();
            exit_info.terminator = Some(exit_yield.into());
        }

        let c42 = ConstI64::new(b, 42);
        let jump = JumpTo::new(
            b,
            Successor::from_block(exit),
            vec![SSAValue::from(c42.result)],
        );
        let entry = b.block().stmt(c42).terminator(jump).new();
        let region = b.region().add_block(entry).add_block(exit).new();
        let body = FunctionDef::new(b, region, Signature::new(vec![], TestType::I64, ()));
        b.specialize().staged_func(staged).body(body).new().unwrap()
    })
}

pub fn build_yield_program(
    pipeline: &mut Pipeline<StageInfo<TestDialect>>,
    stage_id: CompileStage,
) -> kirin_ir::SpecializedFunction {
    let stage = pipeline.stage_mut(stage_id).unwrap();
    stage.with_builder(|b| {
        let staged = b.staged_function().new().unwrap();

        let c7 = ConstI64::new(b, 7);
        let yield_op = YieldOp::new(b, SSAValue::from(c7.result));
        let block = b.block().stmt(c7).terminator(yield_op).new();
        let region = b.region().add_block(block).new();
        let body = FunctionDef::new(b, region, Signature::new(vec![], TestType::I64, ()));
        b.specialize().staged_func(staged).body(body).new().unwrap()
    })
}

pub fn build_return_program(
    pipeline: &mut Pipeline<StageInfo<TestDialect>>,
    stage_id: CompileStage,
) -> kirin_ir::SpecializedFunction {
    let stage = pipeline.stage_mut(stage_id).unwrap();
    stage.with_builder(|b| {
        let staged = b.staged_function().new().unwrap();

        let c11 = ConstI64::new(b, 11);
        let return_op = ReturnOp::new(b, SSAValue::from(c11.result));
        let block = b.block().stmt(c11).terminator(return_op).new();
        let region = b.region().add_block(block).new();
        let body = FunctionDef::new(b, region, Signature::new(vec![], TestType::I64, ()));
        b.specialize().staged_func(staged).body(body).new().unwrap()
    })
}

pub fn build_product_return_program(
    pipeline: &mut Pipeline<StageInfo<TestDialect>>,
    stage_id: CompileStage,
) -> kirin_ir::SpecializedFunction {
    let stage = pipeline.stage_mut(stage_id).unwrap();
    stage.with_builder(|b| {
        let staged = b.staged_function().new().unwrap();

        let c2 = ConstI64::new(b, 2);
        let c40 = ConstI64::new(b, 40);
        let pair = PackTuple::new(b, SSAValue::from(c2.result), SSAValue::from(c40.result));
        let return_op = ReturnOp::new(b, SSAValue::from(pair.result));
        let block = b
            .block()
            .stmt(c2)
            .stmt(c40)
            .stmt(pair)
            .terminator(return_op)
            .new();
        let region = b.region().add_block(block).new();
        let body = FunctionDef::new(b, region, Signature::new(vec![], TestType::Tuple, ()));
        b.specialize().staged_func(staged).body(body).new().unwrap()
    })
}

pub fn build_branch_true_program(
    pipeline: &mut Pipeline<StageInfo<TestDialect>>,
    stage_id: CompileStage,
) -> kirin_ir::SpecializedFunction {
    build_branch_program(pipeline, stage_id, BranchCase::True)
}

pub fn build_branch_false_program(
    pipeline: &mut Pipeline<StageInfo<TestDialect>>,
    stage_id: CompileStage,
) -> kirin_ir::SpecializedFunction {
    build_branch_program(pipeline, stage_id, BranchCase::False)
}

pub fn build_branch_nondeterministic_program(
    pipeline: &mut Pipeline<StageInfo<TestDialect>>,
    stage_id: CompileStage,
) -> kirin_ir::SpecializedFunction {
    build_branch_program(pipeline, stage_id, BranchCase::Unknown)
}

pub fn build_if_program_true(
    pipeline: &mut Pipeline<StageInfo<TestDialect>>,
    stage_id: CompileStage,
) -> kirin_ir::SpecializedFunction {
    build_if_program_with_condition(pipeline, stage_id, IfCondition::True, false)
}

pub fn build_if_program_false(
    pipeline: &mut Pipeline<StageInfo<TestDialect>>,
    stage_id: CompileStage,
) -> kirin_ir::SpecializedFunction {
    build_if_program_with_condition(pipeline, stage_id, IfCondition::False, false)
}

pub fn build_if_program_missing_yield(
    pipeline: &mut Pipeline<StageInfo<TestDialect>>,
    stage_id: CompileStage,
) -> kirin_ir::SpecializedFunction {
    build_if_program_with_condition(pipeline, stage_id, IfCondition::True, true)
}

pub fn build_if_program_nondeterministic(
    pipeline: &mut Pipeline<StageInfo<TestDialect>>,
    stage_id: CompileStage,
) -> kirin_ir::SpecializedFunction {
    build_if_program_with_condition(pipeline, stage_id, IfCondition::Nondeterministic, false)
}

pub fn build_for_program(
    pipeline: &mut Pipeline<StageInfo<TestDialect>>,
    stage_id: CompileStage,
) -> kirin_ir::SpecializedFunction {
    build_for_program_variant(pipeline, stage_id, ForVariant::Success)
}

pub fn build_for_program_missing_yield(
    pipeline: &mut Pipeline<StageInfo<TestDialect>>,
    stage_id: CompileStage,
) -> kirin_ir::SpecializedFunction {
    build_for_program_variant(pipeline, stage_id, ForVariant::MissingYield)
}

pub fn build_for_program_overflow(
    pipeline: &mut Pipeline<StageInfo<TestDialect>>,
    stage_id: CompileStage,
) -> kirin_ir::SpecializedFunction {
    build_for_program_variant(pipeline, stage_id, ForVariant::Overflow)
}

enum IfCondition {
    True,
    False,
    Nondeterministic,
}

enum BranchCase {
    True,
    False,
    Unknown,
}

fn build_branch_program(
    pipeline: &mut Pipeline<StageInfo<TestDialect>>,
    stage_id: CompileStage,
    case: BranchCase,
) -> kirin_ir::SpecializedFunction {
    let stage = pipeline.stage_mut(stage_id).unwrap();
    stage.with_builder(|b| {
        let staged = b.staged_function().new().unwrap();

        let then_value = ConstI64::new(b, 11);
        let then_stop = StopOp::new(b, SSAValue::from(then_value.result));
        let then_block = b.block().stmt(then_value).terminator(then_stop).new();

        let else_value = ConstI64::new(b, 22);
        let else_stop = StopOp::new(b, SSAValue::from(else_value.result));
        let else_block = b.block().stmt(else_value).terminator(else_stop).new();

        let entry = match case {
            BranchCase::True => {
                let condition = ConstI64::new(b, 1);
                let branch = BranchSelect::new(
                    b,
                    SSAValue::from(condition.result),
                    Successor::from_block(then_block),
                    vec![],
                    Successor::from_block(else_block),
                    vec![],
                );
                b.block().stmt(condition).terminator(branch).new()
            }
            BranchCase::False => {
                let condition = ConstI64::new(b, 0);
                let branch = BranchSelect::new(
                    b,
                    SSAValue::from(condition.result),
                    Successor::from_block(then_block),
                    vec![],
                    Successor::from_block(else_block),
                    vec![],
                );
                b.block().stmt(condition).terminator(branch).new()
            }
            BranchCase::Unknown => {
                let condition = UnknownValue::new(b);
                let branch = BranchSelect::new(
                    b,
                    SSAValue::from(condition.result),
                    Successor::from_block(then_block),
                    vec![],
                    Successor::from_block(else_block),
                    vec![],
                );
                b.block().stmt(condition).terminator(branch).new()
            }
        };
        let region = b
            .region()
            .add_block(entry)
            .add_block(then_block)
            .add_block(else_block)
            .new();
        let body = FunctionDef::new(b, region, Signature::new(vec![], TestType::I64, ()));
        b.specialize().staged_func(staged).body(body).new().unwrap()
    })
}

fn build_if_program_with_condition(
    pipeline: &mut Pipeline<StageInfo<TestDialect>>,
    stage_id: CompileStage,
    condition: IfCondition,
    missing_yield: bool,
) -> kirin_ir::SpecializedFunction {
    let stage = pipeline.stage_mut(stage_id).unwrap();
    stage.with_builder(|b| {
        let staged = b.staged_function().new().unwrap();

        let then_value = ConstI64::new(b, 42);
        let then_terminator: kirin_ir::Statement = if missing_yield {
            StopOp::new(b, SSAValue::from(then_value.result)).into()
        } else {
            YieldOp::new(b, SSAValue::from(then_value.result)).into()
        };
        let then_body = b.block().stmt(then_value).terminator(then_terminator).new();

        let else_value = ConstI64::new(b, 7);
        let else_terminator: kirin_ir::Statement = if missing_yield {
            StopOp::new(b, SSAValue::from(else_value.result)).into()
        } else {
            YieldOp::new(b, SSAValue::from(else_value.result)).into()
        };
        let else_body = b.block().stmt(else_value).terminator(else_terminator).new();

        let block = match condition {
            IfCondition::True => {
                let condition_stmt = ConstI64::new(b, 1);
                let if_op = IfOp::new(
                    b,
                    SSAValue::from(condition_stmt.result),
                    then_body,
                    else_body,
                );
                let stop = StopOp::new(b, SSAValue::from(if_op.result));
                b.block()
                    .stmt(condition_stmt)
                    .stmt(if_op)
                    .terminator(stop)
                    .new()
            }
            IfCondition::False => {
                let condition_stmt = ConstI64::new(b, 0);
                let if_op = IfOp::new(
                    b,
                    SSAValue::from(condition_stmt.result),
                    then_body,
                    else_body,
                );
                let stop = StopOp::new(b, SSAValue::from(if_op.result));
                b.block()
                    .stmt(condition_stmt)
                    .stmt(if_op)
                    .terminator(stop)
                    .new()
            }
            IfCondition::Nondeterministic => {
                let lhs = ConstI64::new(b, 1);
                let rhs = ConstI64::new(b, 2);
                let packed =
                    PackTuple::new(b, SSAValue::from(lhs.result), SSAValue::from(rhs.result));
                let if_op = IfOp::new(b, SSAValue::from(packed.result), then_body, else_body);
                let stop = StopOp::new(b, SSAValue::from(if_op.result));
                b.block()
                    .stmt(lhs)
                    .stmt(rhs)
                    .stmt(packed)
                    .stmt(if_op)
                    .terminator(stop)
                    .new()
            }
        };
        let region = b.region().add_block(block).new();
        let body = FunctionDef::new(b, region, Signature::new(vec![], TestType::I64, ()));
        b.specialize().staged_func(staged).body(body).new().unwrap()
    })
}

enum ForVariant {
    Success,
    MissingYield,
    Overflow,
}

fn build_for_program_variant(
    pipeline: &mut Pipeline<StageInfo<TestDialect>>,
    stage_id: CompileStage,
    variant: ForVariant,
) -> kirin_ir::SpecializedFunction {
    let stage = pipeline.stage_mut(stage_id).unwrap();
    stage.with_builder(|b| {
        let staged = b.staged_function().new().unwrap();

        let body = b
            .block()
            .argument(TestType::I64)
            .argument(TestType::I64)
            .new();
        let body_args = &b.block_arena()[body].arguments;
        let iv_arg: SSAValue = body_args[0].into();
        let carried_arg: SSAValue = body_args[1].into();
        let next_carried = AddI64::new(b, iv_arg, carried_arg);
        let body_terminator = match variant {
            ForVariant::Success | ForVariant::Overflow => {
                YieldOp::new(b, SSAValue::from(next_carried.result)).into()
            }
            ForVariant::MissingYield => StopOp::new(b, SSAValue::from(next_carried.result)).into(),
        };
        b.attach_statements_to_block(body, &[next_carried.into()], Some(body_terminator));

        let (start_value, end_value, step_value, init_value) = match variant {
            ForVariant::Success => (0, 3, 1, 10),
            ForVariant::MissingYield => (0, 1, 1, 10),
            ForVariant::Overflow => (i64::MAX - 1, i64::MAX, 2, 0),
        };

        let start = ConstI64::new(b, start_value);
        let end = ConstI64::new(b, end_value);
        let step = ConstI64::new(b, step_value);
        let init = ConstI64::new(b, init_value);
        let for_op = ForOp::new(
            b,
            SSAValue::from(start.result),
            SSAValue::from(end.result),
            SSAValue::from(step.result),
            SSAValue::from(init.result),
            body,
        );
        let stop = StopOp::new(b, SSAValue::from(for_op.result));
        let block = b
            .block()
            .stmt(start)
            .stmt(end)
            .stmt(step)
            .stmt(init)
            .stmt(for_op)
            .terminator(stop)
            .new();
        let region = b.region().add_block(block).new();
        let body = FunctionDef::new(b, region, Signature::new(vec![], TestType::I64, ()));
        let _ = iv_arg;
        b.specialize().staged_func(staged).body(body).new().unwrap()
    })
}
