mod common;

use common::TestDialect;
use kirin_arith::{ArithType, ArithValue};
use kirin_cf::ControlFlow;
use kirin_constant::Constant;
use kirin_function::FunctionBody;
use kirin_interpreter::StackInterpreter;
use kirin_ir::{query::ParentInfo, *};

// ---------------------------------------------------------------------------
// IR builder: select(x) = if x != 0 then x+1 else 42
//
// Uses is_truthy semantics: nonzero → true branch, zero → false branch.
// We add x+1 on the true path to have a statement that produces a result
// (avoiding cross-block block-argument scoping questions).
// ---------------------------------------------------------------------------

fn build_select_program(
    pipeline: &mut Pipeline<StageInfo<TestDialect>>,
    stage_id: CompileStage,
) -> SpecializedFunction {
    let stage = pipeline.stage_mut(stage_id).unwrap();

    let sf = stage.staged_function().new().unwrap();

    // entry block with argument x
    let entry = stage.block().argument(ArithType::I64).new();

    // Get real block argument SSA value
    let x: SSAValue = {
        let si = pipeline.stage(stage_id).unwrap();
        let bi = entry.expect_info(si);
        bi.arguments[0].into()
    };

    let stage = pipeline.stage_mut(stage_id).unwrap();

    // truthy_block: c1 = const 1; sum = add x c1; return sum
    let c1 = Constant::<ArithValue, ArithType>::new(stage, ArithValue::I64(1));
    let sum = kirin_arith::Arith::<ArithType>::op_add(stage, x, c1.result);
    let ret_sum = ControlFlow::<ArithType>::op_return(stage, sum.result);
    let truthy_block = stage.block().stmt(c1).stmt(sum).terminator(ret_sum).new();

    // falsy_block: c42 = const 42; return c42
    let c42 = Constant::<ArithValue, ArithType>::new(stage, ArithValue::I64(42));
    let ret_42 = ControlFlow::<ArithType>::op_return(stage, c42.result);
    let falsy_block = stage.block().stmt(c42).terminator(ret_42).new();

    // Add terminator to entry: cond_br x -> truthy_block, falsy_block
    let cond_br =
        ControlFlow::<ArithType>::op_conditional_branch(stage, x, truthy_block, falsy_block);
    {
        let cond_br_stmt: Statement = cond_br.into();
        // Set the terminator's parent block so the interpreter can resolve block arguments.
        let info = cond_br_stmt.expect_info_mut(stage);
        *info.get_parent_mut() = Some(entry);
        let entry_info: &mut Item<BlockInfo<TestDialect>> = entry.get_info_mut(stage).unwrap();
        entry_info.terminator = Some(cond_br_stmt);
    }

    let region = stage
        .region()
        .add_block(entry)
        .add_block(truthy_block)
        .add_block(falsy_block)
        .new();

    let body = FunctionBody::<ArithType>::new(stage, region);

    stage.specialize().f(sf).body(body).new().unwrap()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn test_concrete_select() {
    let mut pipeline: Pipeline<StageInfo<TestDialect>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();

    let spec_func = build_select_program(&mut pipeline, stage_id);

    let mut interp: StackInterpreter<i64, _> = StackInterpreter::new(&pipeline, stage_id);

    // select(7) → 7+1 = 8 (truthy: nonzero)
    let result = interp.call::<TestDialect>(spec_func, &[7]).unwrap();
    assert_eq!(result, 8);

    // select(-3) → -3+1 = -2 (truthy: nonzero)
    let result = interp.call::<TestDialect>(spec_func, &[-3]).unwrap();
    assert_eq!(result, -2);

    // select(0) → 42 (falsy: zero)
    let result = interp.call::<TestDialect>(spec_func, &[0]).unwrap();
    assert_eq!(result, 42);
}
