mod common;

use common::TestDialect;
use kirin_arith::ArithType;
use kirin_cf::ControlFlow;
use kirin_function::FunctionBody;
use kirin_interpreter::StackInterpreter;
use kirin_ir::*;

// ---------------------------------------------------------------------------
// IR builder: abs(x, y) = if (x - y) < 0 then -(x - y) else (x - y)
// ---------------------------------------------------------------------------

fn build_abs_program(
    pipeline: &mut Pipeline<StageInfo<TestDialect>>,
    stage_id: CompileStage,
) -> SpecializedFunction {
    let stage = pipeline.stage_mut(stage_id).unwrap();

    let sf = stage.staged_function().new().unwrap();

    // Block arguments for the entry block
    let x_ba = stage.block_argument(0);
    let y_ba = stage.block_argument(1);
    let x: SSAValue = x_ba.into();
    let y: SSAValue = y_ba.into();

    // entry: diff = sub(x, y)
    let diff = kirin_arith::Arith::<ArithType>::op_sub(stage, x, y);

    // neg_block: negate diff and return
    let neg_result = kirin_arith::Arith::<ArithType>::op_neg(stage, diff.result);
    let ret_neg = ControlFlow::<ArithType>::op_return(stage, neg_result.result);
    let neg_block = stage.block().stmt(neg_result).terminator(ret_neg).new();

    // non_neg_block: return diff directly
    let ret_pos = ControlFlow::<ArithType>::op_return(stage, diff.result);
    let non_neg_block = stage.block().terminator(ret_pos).new();

    // entry: cond_br diff -> neg_block (if < 0), non_neg_block (if >= 0)
    let cond_br = ControlFlow::<ArithType>::op_conditional_branch(
        stage,
        diff.result,
        neg_block,
        non_neg_block,
    );

    let entry_block = stage
        .block()
        .argument(ArithType::I64)
        .argument(ArithType::I64)
        .stmt(diff)
        .terminator(cond_br)
        .new();

    // Region containing all blocks (entry first)
    let region = stage
        .region()
        .add_block(entry_block)
        .add_block(neg_block)
        .add_block(non_neg_block)
        .new();

    let body = FunctionBody::<ArithType>::new(stage, region);

    stage.specialize().f(sf).body(body).new().unwrap()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn test_concrete_abs() {
    let mut pipeline: Pipeline<StageInfo<TestDialect>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();

    let spec_func = build_abs_program(&mut pipeline, stage_id);

    let mut interp: StackInterpreter<i64, _> = StackInterpreter::new(&pipeline, stage_id);

    // abs(10 - 3) = 7
    let result = interp.call::<TestDialect>(spec_func, &[10, 3]).unwrap();
    assert_eq!(result, 7);

    // abs(3 - 10) = 7
    let result = interp.call::<TestDialect>(spec_func, &[3, 10]).unwrap();
    assert_eq!(result, 7);

    // abs(5 - 5) = 0
    let result = interp.call::<TestDialect>(spec_func, &[5, 5]).unwrap();
    assert_eq!(result, 0);
}
