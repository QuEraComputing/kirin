mod support;

use kirin_interpreter_3::ProductValue;
use kirin_interpreter_3::{
    BlockSeed, Effect, Execute, FunctionSeed, Machine, RegionSeed, ResolutionPolicy, SingleStage,
    StagedFunctionSeed,
};
use kirin_ir::{CompileStage, GetInfo, Pipeline, StageInfo, TestSSAValue, product};
use smallvec::smallvec;

use support::{
    TestDialect, TestMachine, TestValue, build_jump_program, build_product_return_program,
    build_region_yield_program, build_return_program, build_yield_program,
};

fn entry_block(
    pipeline: &Pipeline<StageInfo<TestDialect>>,
    stage_id: CompileStage,
    callee: kirin_ir::SpecializedFunction,
) -> kirin_ir::Block {
    let stage = pipeline.stage(stage_id).unwrap();
    callee
        .expect_info(stage)
        .body()
        .regions(stage)
        .next()
        .unwrap()
        .blocks(stage)
        .next()
        .unwrap()
}

fn entry_region(
    pipeline: &Pipeline<StageInfo<TestDialect>>,
    stage_id: CompileStage,
    callee: kirin_ir::SpecializedFunction,
) -> kirin_ir::Region {
    let stage = pipeline.stage(stage_id).unwrap();
    *callee
        .expect_info(stage)
        .body()
        .regions(stage)
        .next()
        .unwrap()
}

fn second_block(
    pipeline: &Pipeline<StageInfo<TestDialect>>,
    stage_id: CompileStage,
    callee: kirin_ir::SpecializedFunction,
) -> kirin_ir::Block {
    let stage = pipeline.stage(stage_id).unwrap();
    callee
        .expect_info(stage)
        .body()
        .regions(stage)
        .next()
        .unwrap()
        .blocks(stage)
        .nth(1)
        .unwrap()
}

#[test]
fn block_seed_returns_yield_terminal() {
    let mut pipeline: Pipeline<StageInfo<TestDialect>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();
    let callee = build_yield_program(&mut pipeline, stage_id);
    let mut interp = SingleStage::new(&pipeline, stage_id, TestMachine);
    interp.start_specialization(callee, &[]).unwrap();

    let terminal = BlockSeed::entry(entry_block(&pipeline, stage_id, callee))
        .execute(&mut interp)
        .unwrap();

    assert_eq!(terminal, Effect::Yield(TestValue::from(7)));
}

#[test]
fn block_seed_returns_return_terminal() {
    let mut pipeline: Pipeline<StageInfo<TestDialect>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();
    let callee = build_return_program(&mut pipeline, stage_id);
    let mut interp = SingleStage::new(&pipeline, stage_id, TestMachine);
    interp.start_specialization(callee, &[]).unwrap();

    let terminal = BlockSeed::entry(entry_block(&pipeline, stage_id, callee))
        .execute(&mut interp)
        .unwrap();

    assert_eq!(terminal, Effect::Return(TestValue::from(11)));
}

#[test]
fn block_seed_returns_jump_terminal() {
    let mut pipeline: Pipeline<StageInfo<TestDialect>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();
    let callee = build_jump_program(&mut pipeline, stage_id);
    let mut interp = SingleStage::new(&pipeline, stage_id, TestMachine);
    interp.start_specialization(callee, &[]).unwrap();

    let terminal = BlockSeed::entry(entry_block(&pipeline, stage_id, callee))
        .execute(&mut interp)
        .unwrap();

    assert_eq!(
        terminal,
        Effect::Jump(
            second_block(&pipeline, stage_id, callee),
            smallvec![TestValue::from(42)]
        )
    );
}

#[test]
fn region_seed_follows_jump_to_yield() {
    let mut pipeline: Pipeline<StageInfo<TestDialect>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();
    let callee = build_region_yield_program(&mut pipeline, stage_id);
    let mut interp = SingleStage::new(&pipeline, stage_id, TestMachine);
    interp.start_specialization(callee, &[]).unwrap();

    let terminal = RegionSeed::new(entry_region(&pipeline, stage_id, callee), vec![])
        .execute(&mut interp)
        .unwrap();

    assert_eq!(terminal, Effect::Yield(TestValue::from(42)));
}

#[test]
fn function_seed_translates_return_into_bind_product_then_advance() {
    let mut pipeline: Pipeline<StageInfo<TestDialect>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();
    let caller = build_yield_program(&mut pipeline, stage_id);
    let callee = build_return_program(&mut pipeline, stage_id);
    let mut interp = SingleStage::new(&pipeline, stage_id, TestMachine);
    interp.start_specialization(caller, &[]).unwrap();

    let results = product![TestSSAValue(0).into()];
    let effect = FunctionSeed {
        callee,
        args: smallvec![],
        results: results.clone(),
    }
    .execute(&mut interp)
    .unwrap();

    assert_eq!(
        effect,
        Effect::BindProduct(results, TestValue::from(11)).then(Effect::Advance)
    );
}

#[test]
fn function_seed_binds_product_return_values() {
    let mut pipeline: Pipeline<StageInfo<TestDialect>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();
    let caller = build_yield_program(&mut pipeline, stage_id);
    let callee = build_product_return_program(&mut pipeline, stage_id);
    let mut interp = SingleStage::new(&pipeline, stage_id, TestMachine);
    interp.start_specialization(caller, &[]).unwrap();

    let results = product![TestSSAValue(0).into(), TestSSAValue(1).into()];
    let effect = FunctionSeed {
        callee,
        args: smallvec![],
        results: results.clone(),
    }
    .execute(&mut interp)
    .unwrap();

    assert_eq!(
        effect,
        Effect::BindProduct(
            results,
            TestValue::new_product(vec![TestValue::from(2), TestValue::from(40)])
        )
        .then(Effect::Advance)
    );
}

#[test]
fn function_seed_rejects_malformed_bind_product_results() {
    let mut pipeline: Pipeline<StageInfo<TestDialect>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();
    let caller = build_yield_program(&mut pipeline, stage_id);
    let callee = build_return_program(&mut pipeline, stage_id);
    let mut interp = SingleStage::new(&pipeline, stage_id, TestMachine);
    interp.start_specialization(caller, &[]).unwrap();

    let results = product![TestSSAValue(0).into(), TestSSAValue(1).into()];
    let effect = FunctionSeed {
        callee,
        args: smallvec![],
        results,
    }
    .execute(&mut interp)
    .unwrap();
    let error = interp.consume_effect(effect).unwrap_err();

    assert!(matches!(
        error,
        kirin_interpreter_3::InterpError::Interpreter(kirin_interpreter_3::InterpreterError::Unsupported(message)) if message.contains("product index 0 out of bounds")
    ));
}

#[test]
fn staged_function_seed_resolves_unique_live_callee() {
    let mut pipeline: Pipeline<StageInfo<TestDialect>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();
    let function = pipeline.function().name("callee").new().unwrap();
    let staged = pipeline
        .staged_function::<TestDialect>()
        .func(function)
        .stage(stage_id)
        .new()
        .unwrap();
    let caller = build_yield_program(&mut pipeline, stage_id);
    let callee = {
        let stage = pipeline.stage_mut(stage_id).unwrap();
        stage.with_builder(|b| {
            let c11 = support::ConstI64::new(b, 11);
            let return_op = support::ReturnOp::new(b, kirin_ir::SSAValue::from(c11.result));
            let block = b.block().stmt(c11).terminator(return_op).new();
            let region = b.region().add_block(block).new();
            let body = support::FunctionDef::new(
                b,
                region,
                kirin_ir::Signature::new(vec![], support::TestType::I64, ()),
            );
            b.specialize().staged_func(staged).body(body).new().unwrap()
        })
    };
    let mut interp = SingleStage::new(&pipeline, stage_id, TestMachine);
    interp.start_specialization(caller, &[]).unwrap();

    let results = product![TestSSAValue(0).into()];
    let effect = StagedFunctionSeed {
        function,
        args: smallvec![],
        results: results.clone(),
        policy: ResolutionPolicy::UniqueLive,
    }
    .execute(&mut interp)
    .unwrap();

    assert_eq!(
        effect,
        Effect::BindProduct(results, TestValue::from(11)).then(Effect::Advance)
    );
    let _ = callee;
}
