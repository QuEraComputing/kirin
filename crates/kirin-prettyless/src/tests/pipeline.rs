#[test]
fn test_pipeline_function_print() {
    let mut pipeline: Pipeline<kirin_ir::StageInfo<SimpleLanguage>> = Pipeline::new();
    let func = pipeline.function().name("foo").new();

    // --- Stage A: a simple function with one constant ---
    let stage0_id = pipeline
        .add_stage()
        .stage(kirin_ir::StageInfo::default())
        .name("A")
        .new();
    let sf0 = pipeline
        .staged_function()
        .func(func)
        .stage(stage0_id)
        .signature(kirin_ir::Signature {
            params: vec![Int],
            ret: Int,
            constraints: (),
        })
        .new()
        .unwrap();

    let ctx0 = pipeline.stage_mut(stage0_id).unwrap();
    let a = SimpleLanguage::op_constant(ctx0, 42i64);
    let ret = SimpleLanguage::op_return(ctx0, a.result);
    let block = ctx0.block().stmt(a).terminator(ret).new();
    let body = ctx0.region().add_block(block).new();
    let fdef = SimpleLanguage::op_function(ctx0, body);
    ctx0.specialize().f(sf0).body(fdef).new().unwrap();

    // --- Stage B: a different version with two constants ---
    let stage1_id = pipeline
        .add_stage()
        .stage(kirin_ir::StageInfo::default())
        .name("B")
        .new();
    let sf1 = pipeline
        .staged_function()
        .func(func)
        .stage(stage1_id)
        .signature(kirin_ir::Signature {
            params: vec![Int],
            ret: Int,
            constraints: (),
        })
        .new()
        .unwrap();

    let ctx1 = pipeline.stage_mut(stage1_id).unwrap();
    let a = SimpleLanguage::op_constant(ctx1, 10i64);
    let b = SimpleLanguage::op_constant(ctx1, 20i64);
    let c = SimpleLanguage::op_add(ctx1, a.result, b.result);
    let ret = SimpleLanguage::op_return(ctx1, c.result);
    let block = ctx1.block().stmt(a).stmt(b).stmt(c).terminator(ret).new();
    let body = ctx1.region().add_block(block).new();
    let fdef = SimpleLanguage::op_function(ctx1, body);
    ctx1.specialize().f(sf1).body(fdef).new().unwrap();

    // Print the function across both stages
    let output = func.sprint(&pipeline);
    insta::assert_snapshot!(output);
}

#[test]
fn test_pipeline_unnamed_stage() {
    let mut pipeline: Pipeline<kirin_ir::StageInfo<SimpleLanguage>> = Pipeline::new();
    let func = pipeline.function().name("bar").new();

    // --- Unnamed stage (no .name() call) ---
    let stage_id = pipeline
        .add_stage()
        .stage(kirin_ir::StageInfo::default())
        .new();
    let sf = pipeline
        .staged_function()
        .func(func)
        .stage(stage_id)
        .signature(kirin_ir::Signature {
            params: vec![Int, Float],
            ret: Int,
            constraints: (),
        })
        .new()
        .unwrap();

    let ctx = pipeline.stage_mut(stage_id).unwrap();
    let a = SimpleLanguage::op_constant(ctx, 7i64);
    let ret = SimpleLanguage::op_return(ctx, a.result);
    let block = ctx.block().stmt(a).terminator(ret).new();
    let body = ctx.region().add_block(block).new();
    let fdef = SimpleLanguage::op_function(ctx, body);
    ctx.specialize().f(sf).body(fdef).new().unwrap();

    // Should fall back to numeric symbol form: "stage @0"
    let output = func.sprint(&pipeline);
    insta::assert_snapshot!(output);
}

#[test]
fn test_pipeline_staged_function_no_specialization() {
    let mut pipeline: Pipeline<kirin_ir::StageInfo<SimpleLanguage>> = Pipeline::new();
    let func = pipeline.function().name("extern_fn").new();

    // Stage with a named stage but no specialization (declaration-only)
    let stage_id = pipeline
        .add_stage()
        .stage(kirin_ir::StageInfo::default())
        .name("host")
        .new();
    let _sf = pipeline
        .staged_function()
        .func(func)
        .stage(stage_id)
        .signature(kirin_ir::Signature {
            params: vec![Int],
            ret: Float,
            constraints: (),
        })
        .new()
        .unwrap();

    // No specialize() call — staged function has no body / specializations
    let output = func.sprint(&pipeline);
    insta::assert_snapshot!(output);
}
