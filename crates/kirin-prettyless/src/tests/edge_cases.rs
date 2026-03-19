// ============================================================================
// Edge case tests for pretty printing
// ============================================================================

// --- Config edge cases ---

#[test]
fn test_config_zero_width() {
    let config = Config::default().with_width(0);
    assert_eq!(config.max_width, 0);

    // Rendering with zero width should still produce output (each token on its own line)
    let stage: kirin_ir::StageInfo<SimpleLanguage> = kirin_ir::StageInfo::default();
    let v = vec![1i32, 2, 3];
    let output = PrettyPrintExt::<SimpleLanguage>::render(&v, &stage)
        .config(config)
        .into_string()
        .unwrap();
    // Should still contain all values, just possibly wrapped
    assert!(output.contains('1'));
    assert!(output.contains('2'));
    assert!(output.contains('3'));
}

#[test]
fn test_config_zero_tab_spaces() {
    let mut stage: BuilderStageInfo<SimpleLanguage> = BuilderStageInfo::default();

    let a = SimpleLanguage::op_constant(&mut stage, 1i64);
    let ret = SimpleLanguage::op_return(&mut stage, a.result);
    let block = stage.block().stmt(a).terminator(ret).new();

    let config = Config::default().with_tab_spaces(0);
    let stage = stage.finalize().unwrap();
    let doc = Document::new(config, &stage);
    let arena_doc = doc.print_block(&block);
    let mut buf = String::new();
    arena_doc.render_fmt(80, &mut buf).unwrap();
    // With zero tab spaces, indented content is flush left
    insta::assert_snapshot!(buf);
}

#[test]
fn test_config_large_tab_spaces() {
    let mut stage: BuilderStageInfo<SimpleLanguage> = BuilderStageInfo::default();

    let a = SimpleLanguage::op_constant(&mut stage, 1i64);
    let ret = SimpleLanguage::op_return(&mut stage, a.result);
    let block = stage.block().stmt(a).terminator(ret).new();

    let config = Config::default().with_tab_spaces(8);
    let stage = stage.finalize().unwrap();
    let doc = Document::new(config, &stage);
    let arena_doc = doc.print_block(&block);
    let mut buf = String::new();
    arena_doc.render_fmt(80, &mut buf).unwrap();
    insta::assert_snapshot!(buf);
}

// --- Empty pipeline ---

#[test]
fn test_empty_pipeline_render() {
    use crate::PipelinePrintExt;

    let pipeline: Pipeline<kirin_ir::StageInfo<SimpleLanguage>> = Pipeline::new();
    let output = pipeline.sprint();
    assert_eq!(output, "");
}

// --- Pipeline with function but no stages ---

#[test]
fn test_pipeline_function_no_stages() {
    let mut pipeline: Pipeline<kirin_ir::StageInfo<SimpleLanguage>> = Pipeline::new();
    let func = pipeline.function(Some("orphan")).unwrap();

    // Function exists but has no staged representations
    let output = func.sprint(&pipeline);
    assert_eq!(output, "");
}

// --- Unknown function in pipeline ---
// Note: Cannot construct a bogus Function ID from outside kirin-ir (Id is pub(crate)),
// so we test the error variants directly instead.

// --- Block with multiple unnamed arguments ---

#[test]
fn test_print_block_multiple_unnamed_args() {
    let mut stage: BuilderStageInfo<SimpleLanguage> = BuilderStageInfo::default();

    let ret_val = stage.block_argument().index(0);
    let ret = SimpleLanguage::op_return(&mut stage, ret_val);
    let block = stage
        .block()
        .argument(SimpleType::I64)
        .argument(SimpleType::F64)
        .argument(SimpleType::I64)
        .terminator(ret)
        .new();

    let stage = stage.finalize().unwrap();
    let doc = Document::new(Default::default(), &stage);
    let arena_doc = doc.print_block(&block);
    let mut buf = String::new();
    arena_doc.render_fmt(120, &mut buf).unwrap();
    insta::assert_snapshot!(buf);
}

// --- Region with multiple blocks ---

#[test]
fn test_print_region_multiple_blocks() {
    let mut stage: BuilderStageInfo<SimpleLanguage> = BuilderStageInfo::default();

    let a = SimpleLanguage::op_constant(&mut stage, 1i64);
    let ret1 = SimpleLanguage::op_return(&mut stage, a.result);
    let block1 = stage.block().stmt(a).terminator(ret1).new();

    let b = SimpleLanguage::op_constant(&mut stage, 2i64);
    let ret2 = SimpleLanguage::op_return(&mut stage, b.result);
    let block2 = stage.block().stmt(b).terminator(ret2).new();

    let c = SimpleLanguage::op_constant(&mut stage, 3i64);
    let ret3 = SimpleLanguage::op_return(&mut stage, c.result);
    let block3 = stage.block().stmt(c).terminator(ret3).new();

    let region = stage
        .region()
        .add_block(block1)
        .add_block(block2)
        .add_block(block3)
        .new();

    let stage = stage.finalize().unwrap();
    let doc = Document::new(Default::default(), &stage);
    let arena_doc = doc.print_region(&region);
    let mut buf = String::new();
    arena_doc.render_fmt(120, &mut buf).unwrap();
    insta::assert_snapshot!(buf);
}

// --- PrettyPrint for references ---

#[test]
fn test_pretty_print_reference() {
    let stage: kirin_ir::StageInfo<SimpleLanguage> = kirin_ir::StageInfo::default();
    let doc = Document::new(Default::default(), &stage);
    let val = 42i32;
    let ref_val = &val;
    let arena_doc = ref_val.pretty_print(&doc);
    let mut buf = String::new();
    arena_doc.render_fmt(80, &mut buf).unwrap();
    assert_eq!(buf, "42");
}

// --- Float edge cases ---

#[test]
fn test_pretty_print_f64_negative_zero() {
    let stage: kirin_ir::StageInfo<SimpleLanguage> = kirin_ir::StageInfo::default();
    let doc = Document::new(Default::default(), &stage);
    let arena_doc = (-0.0f64).pretty_print(&doc);
    let mut buf = String::new();
    arena_doc.render_fmt(80, &mut buf).unwrap();
    // Negative zero should still render with decimal point
    assert!(buf.contains(".0"), "expected decimal point, got: {}", buf);
}

#[test]
fn test_pretty_print_f64_very_small_fraction() {
    let stage: kirin_ir::StageInfo<SimpleLanguage> = kirin_ir::StageInfo::default();
    let doc = Document::new(Default::default(), &stage);
    let arena_doc = 0.000001f64.pretty_print(&doc);
    let mut buf = String::new();
    arena_doc.render_fmt(80, &mut buf).unwrap();
    assert_eq!(buf, "0.000001");
}

#[test]
fn test_pretty_print_f32_negative_zero() {
    let stage: kirin_ir::StageInfo<SimpleLanguage> = kirin_ir::StageInfo::default();
    let doc = Document::new(Default::default(), &stage);
    let arena_doc = (-0.0f32).pretty_print(&doc);
    let mut buf = String::new();
    arena_doc.render_fmt(80, &mut buf).unwrap();
    assert!(buf.contains(".0"), "expected decimal point, got: {}", buf);
}

// --- String edge cases ---

#[test]
fn test_pretty_print_string_with_quotes() {
    let stage: kirin_ir::StageInfo<SimpleLanguage> = kirin_ir::StageInfo::default();
    let doc = Document::new(Default::default(), &stage);
    let arena_doc = String::from("say \"hello\"").pretty_print(&doc);
    let mut buf = String::new();
    arena_doc.render_fmt(80, &mut buf).unwrap();
    // {:?} escapes inner quotes properly
    assert_eq!(buf, r#""say \"hello\"""#);
}

#[test]
fn test_pretty_print_string_with_newline() {
    let stage: kirin_ir::StageInfo<SimpleLanguage> = kirin_ir::StageInfo::default();
    let doc = Document::new(Default::default(), &stage);
    let arena_doc = String::from("line1\nline2").pretty_print(&doc);
    let mut buf = String::new();
    arena_doc.render_fmt(80, &mut buf).unwrap();
    // Newlines are now escaped
    assert!(!buf.contains('\n') || buf == "\"line1\\nline2\"");
}

// --- Option<T> pretty print with nested option ---

#[test]
fn test_pretty_print_option_some_string() {
    let stage: kirin_ir::StageInfo<SimpleLanguage> = kirin_ir::StageInfo::default();
    let doc = Document::new(Default::default(), &stage);
    let v: Option<String> = Some("hello".to_string());
    let arena_doc = v.pretty_print(&doc);
    let mut buf = String::new();
    arena_doc.render_fmt(80, &mut buf).unwrap();
    assert_eq!(buf, "\"hello\"");
}

// --- Vec of vecs ---

#[test]
fn test_pretty_print_vec_of_vecs() {
    let stage: kirin_ir::StageInfo<SimpleLanguage> = kirin_ir::StageInfo::default();
    let doc = Document::new(Default::default(), &stage);
    let v: Vec<Vec<i32>> = vec![vec![1, 2], vec![3]];
    let arena_doc = v.pretty_print(&doc);
    let mut buf = String::new();
    arena_doc.render_fmt(80, &mut buf).unwrap();
    assert_eq!(buf, "1, 2, 3");
}

// --- RenderError ---

#[test]
fn test_render_error_display_io() {
    let err = crate::RenderError::Io(std::io::Error::new(
        std::io::ErrorKind::BrokenPipe,
        "pipe broken",
    ));
    let msg = err.to_string();
    assert!(msg.contains("I/O error"), "got: {}", msg);
    assert!(msg.contains("pipe broken"), "got: {}", msg);
}

#[test]
fn test_render_error_display_fmt() {
    let err = crate::RenderError::Fmt(std::fmt::Error);
    let msg = err.to_string();
    assert!(msg.contains("formatting error"), "got: {}", msg);
}

#[test]
fn test_render_error_display_unknown_function() {
    // Create a real function in a pipeline, then check the error variant
    let mut pipeline: Pipeline<kirin_ir::StageInfo<SimpleLanguage>> = Pipeline::new();
    let func = pipeline.function(Some("missing")).unwrap();
    let err = crate::RenderError::UnknownFunction(func);
    let msg = err.to_string();
    assert!(msg.contains("not found in pipeline"), "got: {}", msg);
}

#[test]
fn test_render_error_source() {
    use std::error::Error;

    let io_err = crate::RenderError::Io(std::io::Error::other("test"));
    assert!(io_err.source().is_some());

    let fmt_err = crate::RenderError::Fmt(std::fmt::Error);
    assert!(fmt_err.source().is_some());

    let mut pipeline: Pipeline<kirin_ir::StageInfo<SimpleLanguage>> = Pipeline::new();
    let func = pipeline.function(Some("src_test")).unwrap();
    let unk_err = crate::RenderError::UnknownFunction(func);
    assert!(unk_err.source().is_none());
}

// --- Unnamed function in staged function header ---

#[test]
fn test_staged_function_unnamed() {
    let mut pipeline: Pipeline<kirin_ir::StageInfo<SimpleLanguage>> = Pipeline::new();
    // Create function without a name
    let func = pipeline.function(None::<&str>).unwrap();

    let stage_id = pipeline.add_stage(kirin_ir::StageInfo::default(), Some("X"));
    let sf = pipeline
        .staged_function::<SimpleLanguage>(func, stage_id, Some(kirin_ir::Signature::new(vec![], SimpleType::I64, ())), None, None)
        .unwrap();

    pipeline.stage_mut(stage_id).unwrap().with_builder(|ctx| {
        let a = SimpleLanguage::op_constant(ctx, 0i64);
        let ret = SimpleLanguage::op_return(ctx, a.result);
        let block = ctx.block().stmt(a).terminator(ret).new();
        let body = ctx.region().add_block(block).new();
        let fdef = SimpleLanguage::op_function(ctx, body);
        ctx.specialize(sf, None, fdef, None).unwrap();
    });

    let output = func.sprint(&pipeline);
    // Should contain "unnamed" since no name was set
    insta::assert_snapshot!(output);
}

// --- Staged function with empty params ---

#[test]
fn test_staged_function_no_params() {
    let mut gs: InternTable<String, GlobalSymbol> = InternTable::default();
    let test_func = gs.intern("nullary".to_string());
    let mut stage: BuilderStageInfo<SimpleLanguage> = BuilderStageInfo::default();
    let staged_function = stage
        .staged_function(Some(test_func), Some(kirin_ir::Signature::new(vec![], SimpleType::I64, ())), None, None)
        .unwrap();

    let a = SimpleLanguage::op_constant(&mut stage, 0i64);
    let ret = SimpleLanguage::op_return(&mut stage, a.result);
    let block = stage.block().stmt(a).terminator(ret).new();
    let body = stage.region().add_block(block).new();
    let fdef = SimpleLanguage::op_function(&mut stage, body);
    let _ = stage.specialize(staged_function, None, fdef, None).unwrap();

    let stage = stage.finalize().unwrap();
    let output = staged_function.render(&stage).globals(&gs).into_string().unwrap();
    insta::assert_snapshot!(output);
}

// --- Document list with custom separator ---

#[test]
fn test_document_list_custom_separator() {
    let stage: kirin_ir::StageInfo<SimpleLanguage> = kirin_ir::StageInfo::default();
    let doc = Document::new(Default::default(), &stage);

    let items = [1, 2, 3];
    let result = doc.list(items.iter(), " | ", |i| doc.text(format!("{}", i)));
    let mut buf = String::new();
    result.render_fmt(80, &mut buf).unwrap();
    assert_eq!(buf, "1 | 2 | 3");
}

// --- PipelineRenderBuilder with custom config ---

#[test]
fn test_pipeline_render_builder_write_to() {
    use crate::PipelinePrintExt;

    let mut pipeline: Pipeline<kirin_ir::StageInfo<SimpleLanguage>> = Pipeline::new();
    let func = pipeline.function(Some("wr")).unwrap();

    let stage_id = pipeline.add_stage(kirin_ir::StageInfo::default(), Some("S"));
    let sf = pipeline
        .staged_function::<SimpleLanguage>(func, stage_id, Some(kirin_ir::Signature::new(vec![SimpleType::I64], SimpleType::I64, ())), None, None)
        .unwrap();

    pipeline.stage_mut(stage_id).unwrap().with_builder(|ctx| {
        let a = SimpleLanguage::op_constant(ctx, 5i64);
        let ret = SimpleLanguage::op_return(ctx, a.result);
        let block = ctx.block().stmt(a).terminator(ret).new();
        let body = ctx.region().add_block(block).new();
        let fdef = SimpleLanguage::op_function(ctx, body);
        ctx.specialize(sf, None, fdef, None).unwrap();
    });

    let mut output = Vec::new();
    pipeline.render().write_to(&mut output).unwrap();
    let text = String::from_utf8(output).unwrap();
    assert!(text.contains("stage @S"), "got: {}", text);
    assert!(text.contains("@wr"), "got: {}", text);
}

// --- FunctionRenderBuilder write_to ---

#[test]
fn test_function_render_builder_write_to() {
    let mut pipeline: Pipeline<kirin_ir::StageInfo<SimpleLanguage>> = Pipeline::new();
    let func = pipeline.function(Some("fwr")).unwrap();

    let stage_id = pipeline.add_stage(kirin_ir::StageInfo::default(), Some("T"));
    let sf = pipeline
        .staged_function::<SimpleLanguage>(func, stage_id, Some(kirin_ir::Signature::new(vec![], SimpleType::I64, ())), None, None)
        .unwrap();

    pipeline.stage_mut(stage_id).unwrap().with_builder(|ctx| {
        let a = SimpleLanguage::op_constant(ctx, 99i64);
        let ret = SimpleLanguage::op_return(ctx, a.result);
        let block = ctx.block().stmt(a).terminator(ret).new();
        let body = ctx.region().add_block(block).new();
        let fdef = SimpleLanguage::op_function(ctx, body);
        ctx.specialize(sf, None, fdef, None).unwrap();
    });

    let mut output = Vec::new();
    func.render(&pipeline).write_to(&mut output).unwrap();
    let text = String::from_utf8(output).unwrap();
    assert!(text.contains("stage @T"), "got: {}", text);
    assert!(text.contains("@fwr"), "got: {}", text);
}

// --- Render with very narrow width forces wrapping ---

#[test]
fn test_render_very_narrow_width() {
    let mut gs: InternTable<String, GlobalSymbol> = InternTable::default();
    let test_func = gs.intern("narrow".to_string());
    let mut stage: BuilderStageInfo<SimpleLanguage> = BuilderStageInfo::default();
    let sf = stage
        .staged_function(Some(test_func), Some(kirin_ir::Signature::new(vec![SimpleType::I64, SimpleType::F64, SimpleType::I64], SimpleType::F64, ())), None, None)
        .unwrap();

    let a = SimpleLanguage::op_constant(&mut stage, 1i64);
    let ret = SimpleLanguage::op_return(&mut stage, a.result);
    let block = stage.block().stmt(a).terminator(ret).new();
    let body = stage.region().add_block(block).new();
    let fdef = SimpleLanguage::op_function(&mut stage, body);
    let _ = stage.specialize(sf, None, fdef, None).unwrap();

    let stage = stage.finalize().unwrap();
    let output = sf
        .render(&stage)
        .config(Config::default().with_width(10))
        .globals(&gs)
        .into_string()
        .unwrap();
    // Just verify it doesn't panic and produces output
    assert!(!output.is_empty());
    assert!(output.contains("narrow"));
}
