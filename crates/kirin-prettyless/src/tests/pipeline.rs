#[test]
fn test_pipeline_function_print() {
    let mut pipeline: Pipeline<kirin_ir::StageInfo<SimpleLanguage>> = Pipeline::new();
    let func = pipeline.function(Some("foo")).unwrap();

    // --- Stage A: a simple function with one constant ---
    let stage0_id = pipeline.add_stage(kirin_ir::StageInfo::default(), Some("A"));
    let sf0 = pipeline
        .staged_function::<SimpleLanguage>(func, stage0_id, Some(kirin_ir::Signature::new(vec![SimpleType::I64], SimpleType::I64, ())), None, None)
        .unwrap();

    pipeline.stage_mut(stage0_id).unwrap().with_builder(|ctx0| {
        let a = SimpleLanguage::op_constant(ctx0, 42i64);
        let ret = SimpleLanguage::op_return(ctx0, a.result);
        let block = ctx0.block().stmt(a).terminator(ret).new();
        let body = ctx0.region().add_block(block).new();
        let fdef = SimpleLanguage::op_function(ctx0, body);
        ctx0.specialize(sf0, None, fdef, None).unwrap();
    });

    // --- Stage B: a different version with two constants ---
    let stage1_id = pipeline.add_stage(kirin_ir::StageInfo::default(), Some("B"));
    let sf1 = pipeline
        .staged_function::<SimpleLanguage>(func, stage1_id, Some(kirin_ir::Signature::new(vec![SimpleType::I64], SimpleType::I64, ())), None, None)
        .unwrap();

    pipeline.stage_mut(stage1_id).unwrap().with_builder(|ctx1| {
        let a = SimpleLanguage::op_constant(ctx1, 10i64);
        let b = SimpleLanguage::op_constant(ctx1, 20i64);
        let c = SimpleLanguage::op_add(ctx1, a.result, b.result);
        let ret = SimpleLanguage::op_return(ctx1, c.result);
        let block = ctx1.block().stmt(a).stmt(b).stmt(c).terminator(ret).new();
        let body = ctx1.region().add_block(block).new();
        let fdef = SimpleLanguage::op_function(ctx1, body);
        ctx1.specialize(sf1, None, fdef, None).unwrap();
    });

    // Print the function across both stages
    let output = func.sprint(&pipeline);
    insta::assert_snapshot!(output);
}

#[test]
fn test_pipeline_unnamed_stage() {
    let mut pipeline: Pipeline<kirin_ir::StageInfo<SimpleLanguage>> = Pipeline::new();
    let func = pipeline.function(Some("bar")).unwrap();

    // --- Unnamed stage (no name) ---
    let stage_id = pipeline.add_stage(kirin_ir::StageInfo::default(), None::<&str>);
    let sf = pipeline
        .staged_function::<SimpleLanguage>(func, stage_id, Some(kirin_ir::Signature::new(vec![SimpleType::I64, SimpleType::F64], SimpleType::I64, ())), None, None)
        .unwrap();

    pipeline.stage_mut(stage_id).unwrap().with_builder(|ctx| {
        let a = SimpleLanguage::op_constant(ctx, 7i64);
        let ret = SimpleLanguage::op_return(ctx, a.result);
        let block = ctx.block().stmt(a).terminator(ret).new();
        let body = ctx.region().add_block(block).new();
        let fdef = SimpleLanguage::op_function(ctx, body);
        ctx.specialize(sf, None, fdef, None).unwrap();
    });

    // Should fall back to numeric symbol form: "stage @0"
    let output = func.sprint(&pipeline);
    insta::assert_snapshot!(output);
}

#[test]
fn test_pipeline_staged_function_no_specialization() {
    let mut pipeline: Pipeline<kirin_ir::StageInfo<SimpleLanguage>> = Pipeline::new();
    let func = pipeline.function(Some("extern_fn")).unwrap();

    // Stage with a named stage but no specialization (declaration-only)
    let stage_id = pipeline.add_stage(kirin_ir::StageInfo::default(), Some("host"));
    let _sf = pipeline
        .staged_function::<SimpleLanguage>(func, stage_id, Some(kirin_ir::Signature::new(vec![SimpleType::I64], SimpleType::F64, ())), None, None)
        .unwrap();

    // No specialize() call — staged function has no body / specializations
    let output = func.sprint(&pipeline);
    insta::assert_snapshot!(output);
}
