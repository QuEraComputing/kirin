use kirin_arith::{ArithType, ArithValue};
use kirin_cf::ControlFlow;
use kirin_constant::Constant;
use kirin_function::FunctionBody;
use kirin_interpreter::StackInterpreter;
use kirin_ir::{query::ParentInfo, *};
use kirin_test_utils::{CompositeLanguage, dump_function};

// ---------------------------------------------------------------------------
// IR builder: select(x) = if x != 0 then x+1 else 42
//
// Uses block arguments to pass values across control flow edges:
//
//   entry(x):
//     c1 = const 1
//     sum = add x, c1          // x + 1
//     c42 = const 42
//     cond_br x then=truthy_block(sum) else=falsy_block(c42)
//   truthy_block(val):          // receives sum via block arg
//     ret val
//   falsy_block(val):           // receives 42 via block arg
//     ret val
// ---------------------------------------------------------------------------

fn build_select_program(
    pipeline: &mut Pipeline<StageInfo<CompositeLanguage>>,
    stage_id: CompileStage,
) -> SpecializedFunction {
    let stage = pipeline.stage_mut(stage_id).unwrap();

    let sf = stage.staged_function().new().unwrap();

    // entry block with argument x
    let entry = stage.block().argument(ArithType::I64).new();

    // Get real block argument SSA value for x
    let x: SSAValue = {
        let si = pipeline.stage(stage_id).unwrap();
        let bi = entry.expect_info(si);
        bi.arguments[0].into()
    };

    let stage = pipeline.stage_mut(stage_id).unwrap();

    // truthy_block(val): receives sum via block arg, returns val
    let truthy_block = stage.block().argument(ArithType::I64).new();
    let truthy_val: SSAValue = {
        let si = pipeline.stage(stage_id).unwrap();
        let bi = truthy_block.expect_info(si);
        bi.arguments[0].into()
    };

    let stage = pipeline.stage_mut(stage_id).unwrap();
    let ret_truthy = ControlFlow::<ArithType>::op_return(stage, truthy_val);
    {
        let truthy_info: &mut Item<BlockInfo<CompositeLanguage>> =
            truthy_block.get_info_mut(stage).unwrap();
        truthy_info.terminator = Some(ret_truthy.into());
    }

    // falsy_block(val): receives c42 via block arg, returns val
    let falsy_block = stage.block().argument(ArithType::I64).new();
    let falsy_val: SSAValue = {
        let si = pipeline.stage(stage_id).unwrap();
        let bi = falsy_block.expect_info(si);
        bi.arguments[0].into()
    };

    let stage = pipeline.stage_mut(stage_id).unwrap();
    let ret_falsy = ControlFlow::<ArithType>::op_return(stage, falsy_val);
    {
        let falsy_info: &mut Item<BlockInfo<CompositeLanguage>> =
            falsy_block.get_info_mut(stage).unwrap();
        falsy_info.terminator = Some(ret_falsy.into());
    }

    // Compute values in entry block before branching
    let c1 = Constant::<ArithValue, ArithType>::new(stage, ArithValue::I64(1));
    let sum = kirin_arith::Arith::<ArithType>::op_add(stage, x, c1.result);
    let c42 = Constant::<ArithValue, ArithType>::new(stage, ArithValue::I64(42));

    // Add terminator to entry: cond_br x -> truthy_block(sum), falsy_block(c42)
    let cond_br = ControlFlow::<ArithType>::op_conditional_branch(
        stage,
        x,
        truthy_block,
        vec![sum.result.into()],
        falsy_block,
        vec![c42.result.into()],
    );

    // Wire up entry block: statements + terminator
    {
        let stmts: Vec<Statement> = vec![c1.into(), sum.into(), c42.into()];
        for &s in &stmts {
            let info = s.expect_info_mut(stage);
            *info.get_parent_mut() = Some(entry);
        }
        let linked = stage.link_statements(&stmts);

        let cond_br_stmt: Statement = cond_br.into();
        let info = cond_br_stmt.expect_info_mut(stage);
        *info.get_parent_mut() = Some(entry);

        let entry_info: &mut Item<BlockInfo<CompositeLanguage>> = entry.get_info_mut(stage).unwrap();
        entry_info.statements = linked;
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
fn test_select_ir_snapshot() {
    let mut pipeline: Pipeline<StageInfo<CompositeLanguage>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();

    let spec_func = build_select_program(&mut pipeline, stage_id);
    let ir = dump_function(spec_func, &pipeline, stage_id);
    insta::assert_snapshot!(ir);
}

#[test]
fn test_concrete_select() {
    let mut pipeline: Pipeline<StageInfo<CompositeLanguage>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();

    let spec_func = build_select_program(&mut pipeline, stage_id);

    let mut interp: StackInterpreter<i64, _> = StackInterpreter::new(&pipeline, stage_id);

    // select(7) → 7+1 = 8 (truthy: nonzero)
    let result = interp.call::<CompositeLanguage>(spec_func, &[7]).unwrap();
    assert_eq!(result, 8);

    // select(-3) → -3+1 = -2 (truthy: nonzero)
    let result = interp.call::<CompositeLanguage>(spec_func, &[-3]).unwrap();
    assert_eq!(result, -2);

    // select(0) → 42 (falsy: zero)
    let result = interp.call::<CompositeLanguage>(spec_func, &[0]).unwrap();
    assert_eq!(result, 42);
}
