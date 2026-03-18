// ============================================================================
// PrettyPrint impl tests for builtin types
// ============================================================================

// --- bool ---

#[test]
fn test_pretty_print_bool_true() {
    let stage: kirin_ir::StageInfo<SimpleLanguage> = kirin_ir::StageInfo::default();
    let doc = Document::new(Default::default(), &stage);
    let arena_doc = true.pretty_print(&doc);
    let mut buf = String::new();
    arena_doc.render_fmt(80, &mut buf).unwrap();
    assert_eq!(buf, "true");
}

#[test]
fn test_pretty_print_bool_false() {
    let stage: kirin_ir::StageInfo<SimpleLanguage> = kirin_ir::StageInfo::default();
    let doc = Document::new(Default::default(), &stage);
    let arena_doc = false.pretty_print(&doc);
    let mut buf = String::new();
    arena_doc.render_fmt(80, &mut buf).unwrap();
    assert_eq!(buf, "false");
}

// --- String ---

#[test]
fn test_pretty_print_string() {
    let stage: kirin_ir::StageInfo<SimpleLanguage> = kirin_ir::StageInfo::default();
    let doc = Document::new(Default::default(), &stage);
    let arena_doc = String::from("hello").pretty_print(&doc);
    let mut buf = String::new();
    arena_doc.render_fmt(80, &mut buf).unwrap();
    assert_eq!(buf, "\"hello\"");
}

#[test]
fn test_pretty_print_string_empty() {
    let stage: kirin_ir::StageInfo<SimpleLanguage> = kirin_ir::StageInfo::default();
    let doc = Document::new(Default::default(), &stage);
    let arena_doc = String::new().pretty_print(&doc);
    let mut buf = String::new();
    arena_doc.render_fmt(80, &mut buf).unwrap();
    assert_eq!(buf, "\"\"");
}

// --- f32 ---

#[test]
fn test_pretty_print_f32_whole_number() {
    let stage: kirin_ir::StageInfo<SimpleLanguage> = kirin_ir::StageInfo::default();
    let doc = Document::new(Default::default(), &stage);
    let arena_doc = 1.0f32.pretty_print(&doc);
    let mut buf = String::new();
    arena_doc.render_fmt(80, &mut buf).unwrap();
    assert_eq!(buf, "1.0");
}

#[test]
fn test_pretty_print_f32_fractional() {
    let stage: kirin_ir::StageInfo<SimpleLanguage> = kirin_ir::StageInfo::default();
    let doc = Document::new(Default::default(), &stage);
    let arena_doc = 1.23f32.pretty_print(&doc);
    let mut buf = String::new();
    arena_doc.render_fmt(80, &mut buf).unwrap();
    assert_eq!(buf, "1.23");
}

// --- f64 ---

#[test]
fn test_pretty_print_f64_whole_number() {
    let stage: kirin_ir::StageInfo<SimpleLanguage> = kirin_ir::StageInfo::default();
    let doc = Document::new(Default::default(), &stage);
    let arena_doc = 42.0f64.pretty_print(&doc);
    let mut buf = String::new();
    arena_doc.render_fmt(80, &mut buf).unwrap();
    assert_eq!(buf, "42.0");
}

#[test]
fn test_pretty_print_f64_fractional() {
    let stage: kirin_ir::StageInfo<SimpleLanguage> = kirin_ir::StageInfo::default();
    let doc = Document::new(Default::default(), &stage);
    let arena_doc = 1.234f64.pretty_print(&doc);
    let mut buf = String::new();
    arena_doc.render_fmt(80, &mut buf).unwrap();
    assert_eq!(buf, "1.234");
}

#[test]
fn test_pretty_print_f64_zero() {
    let stage: kirin_ir::StageInfo<SimpleLanguage> = kirin_ir::StageInfo::default();
    let doc = Document::new(Default::default(), &stage);
    let arena_doc = 0.0f64.pretty_print(&doc);
    let mut buf = String::new();
    arena_doc.render_fmt(80, &mut buf).unwrap();
    assert_eq!(buf, "0.0");
}

// --- Vec<T> ---

#[test]
fn test_pretty_print_vec_empty() {
    let stage: kirin_ir::StageInfo<SimpleLanguage> = kirin_ir::StageInfo::default();
    let doc = Document::new(Default::default(), &stage);
    let v: Vec<i32> = vec![];
    let arena_doc = v.pretty_print(&doc);
    let mut buf = String::new();
    arena_doc.render_fmt(80, &mut buf).unwrap();
    assert_eq!(buf, "");
}

#[test]
fn test_pretty_print_vec_single() {
    let stage: kirin_ir::StageInfo<SimpleLanguage> = kirin_ir::StageInfo::default();
    let doc = Document::new(Default::default(), &stage);
    let v = vec![42i32];
    let arena_doc = v.pretty_print(&doc);
    let mut buf = String::new();
    arena_doc.render_fmt(80, &mut buf).unwrap();
    assert_eq!(buf, "42");
}

#[test]
fn test_pretty_print_vec_multiple() {
    let stage: kirin_ir::StageInfo<SimpleLanguage> = kirin_ir::StageInfo::default();
    let doc = Document::new(Default::default(), &stage);
    let v = vec![1i32, 2, 3];
    let arena_doc = v.pretty_print(&doc);
    let mut buf = String::new();
    arena_doc.render_fmt(80, &mut buf).unwrap();
    assert_eq!(buf, "1, 2, 3");
}

// --- Option<T> ---

#[test]
fn test_pretty_print_option_none() {
    let stage: kirin_ir::StageInfo<SimpleLanguage> = kirin_ir::StageInfo::default();
    let doc = Document::new(Default::default(), &stage);
    let v: Option<i32> = None;
    let arena_doc = v.pretty_print(&doc);
    let mut buf = String::new();
    arena_doc.render_fmt(80, &mut buf).unwrap();
    assert_eq!(buf, "");
}

#[test]
fn test_pretty_print_option_some() {
    let stage: kirin_ir::StageInfo<SimpleLanguage> = kirin_ir::StageInfo::default();
    let doc = Document::new(Default::default(), &stage);
    let v: Option<i32> = Some(99);
    let arena_doc = v.pretty_print(&doc);
    let mut buf = String::new();
    arena_doc.render_fmt(80, &mut buf).unwrap();
    assert_eq!(buf, "99");
}

// --- Integer types ---

#[test]
fn test_pretty_print_i8() {
    let stage: kirin_ir::StageInfo<SimpleLanguage> = kirin_ir::StageInfo::default();
    let doc = Document::new(Default::default(), &stage);
    let arena_doc = (-42i8).pretty_print(&doc);
    let mut buf = String::new();
    arena_doc.render_fmt(80, &mut buf).unwrap();
    assert_eq!(buf, "-42");
}

#[test]
fn test_pretty_print_u64() {
    let stage: kirin_ir::StageInfo<SimpleLanguage> = kirin_ir::StageInfo::default();
    let doc = Document::new(Default::default(), &stage);
    let arena_doc = 18446744073709551615u64.pretty_print(&doc);
    let mut buf = String::new();
    arena_doc.render_fmt(80, &mut buf).unwrap();
    assert_eq!(buf, "18446744073709551615");
}

// --- Symbol (local) ---

#[test]
fn test_pretty_print_symbol_resolved() {
    let mut stage: BuilderStageInfo<SimpleLanguage> = BuilderStageInfo::default();
    let sym = stage.symbol_table_mut().intern("my_var".to_string());
    let doc = Document::new(Default::default(), &stage);
    let arena_doc = sym.pretty_print(&doc);
    let mut buf = String::new();
    arena_doc.render_fmt(80, &mut buf).unwrap();
    assert_eq!(buf, "@my_var");
}

// --- Successor ---

#[test]
fn test_pretty_print_successor() {
    use kirin_ir::Successor;
    let mut stage: BuilderStageInfo<SimpleLanguage> = BuilderStageInfo::default();
    let block = stage.block().new();
    let succ = Successor::from_block(block);
    let doc = Document::new(Default::default(), &stage);
    let arena_doc = succ.pretty_print(&doc);
    let mut buf = String::new();
    arena_doc.render_fmt(80, &mut buf).unwrap();
    // Successor renders via Display which is ^<raw_id>
    assert!(buf.starts_with("^"), "expected '^' prefix, got: {}", buf);
}

// ============================================================================
// Document method tests
// ============================================================================

#[test]
fn test_block_indent() {
    let stage: kirin_ir::StageInfo<SimpleLanguage> = kirin_ir::StageInfo::default();
    let config = Config::default().with_tab_spaces(4);
    let doc = Document::new(config, &stage);
    let inner = doc.text("stmt1;") + doc.line_() + doc.text("stmt2;");
    let result = doc.block_indent(inner);
    let mut buf = String::new();
    // Use narrow width to force line breaks
    result.render_fmt(10, &mut buf).unwrap();
    insta::assert_snapshot!(buf);
}

// ============================================================================
// print_block tests
// ============================================================================

#[test]
fn test_print_block_empty_body() {
    let mut gs: InternTable<String, GlobalSymbol> = InternTable::default();
    let test_sym = gs.intern("test".to_string());
    let mut stage: BuilderStageInfo<SimpleLanguage> = BuilderStageInfo::default();
    let _ = stage.staged_function().name(test_sym).new().unwrap();

    let block = stage.block().new();
    let doc = Document::new(Default::default(), &stage);
    let arena_doc = doc.print_block(&block);
    let mut buf = String::new();
    arena_doc.render_fmt(80, &mut buf).unwrap();
    insta::assert_snapshot!(buf);
}

#[test]
fn test_print_block_only_terminator() {
    let mut gs: InternTable<String, GlobalSymbol> = InternTable::default();
    let test_sym = gs.intern("test".to_string());
    let mut stage: BuilderStageInfo<SimpleLanguage> = BuilderStageInfo::default();
    let _ = stage.staged_function().name(test_sym).new().unwrap();

    let a = SimpleLanguage::op_constant(&mut stage, 1i64);
    let ret = SimpleLanguage::op_return(&mut stage, a.result);
    let block = stage.block().stmt(a).terminator(ret).new();

    let doc = Document::new(Default::default(), &stage);
    let arena_doc = doc.print_block(&block);
    let mut buf = String::new();
    arena_doc.render_fmt(80, &mut buf).unwrap();
    insta::assert_snapshot!(buf);
}

#[test]
fn test_print_block_with_named_args() {
    let mut gs: InternTable<String, GlobalSymbol> = InternTable::default();
    let test_sym = gs.intern("test".to_string());
    let mut stage: BuilderStageInfo<SimpleLanguage> = BuilderStageInfo::default();
    let _ = stage.staged_function().name(test_sym).new().unwrap();

    let ret_val = stage.block_argument().index(0);
    let ret = SimpleLanguage::op_return(&mut stage, ret_val);
    let block = stage
        .block()
        .argument(SimpleType::I64)
        .arg_name("x")
        .terminator(ret)
        .new();

    let doc = Document::new(Default::default(), &stage);
    let arena_doc = doc.print_block(&block);
    let mut buf = String::new();
    arena_doc.render_fmt(80, &mut buf).unwrap();
    insta::assert_snapshot!(buf);
}

// ============================================================================
// print_region tests
// ============================================================================

#[test]
fn test_print_region_empty() {
    let mut gs: InternTable<String, GlobalSymbol> = InternTable::default();
    let test_sym = gs.intern("test".to_string());
    let mut stage: BuilderStageInfo<SimpleLanguage> = BuilderStageInfo::default();
    let _ = stage.staged_function().name(test_sym).new().unwrap();

    let region = stage.region().new();
    let doc = Document::new(Default::default(), &stage);
    let arena_doc = doc.print_region(&region);
    let mut buf = String::new();
    arena_doc.render_fmt(80, &mut buf).unwrap();
    insta::assert_snapshot!(buf);
}

// ============================================================================
// RenderBuilder tests
// ============================================================================

#[test]
fn test_render_builder_write_to() {
    let stage: kirin_ir::StageInfo<SimpleLanguage> = kirin_ir::StageInfo::default();
    let value = 42i32;
    let mut output = Vec::new();
    let _ = PrettyPrintExt::<SimpleLanguage>::render(&value, &stage)
        .write_to(&mut output);
    assert_eq!(String::from_utf8(output).unwrap(), "42\n");
}

#[test]
fn test_render_builder_config() {
    let mut gs: InternTable<String, GlobalSymbol> = InternTable::default();
    let test_sym = gs.intern("test".to_string());
    let mut stage: BuilderStageInfo<SimpleLanguage> = BuilderStageInfo::default();
    let sf = stage
        .staged_function()
        .name(test_sym)
        .signature(kirin_ir::Signature {
            params: vec![SimpleType::I64],
            ret: SimpleType::I64,
            constraints: (),
        })
        .new()
        .unwrap();

    let a = SimpleLanguage::op_constant(&mut stage, 1i64);
    let b = SimpleLanguage::op_constant(&mut stage, 2i64);
    let c = SimpleLanguage::op_add(&mut stage, a.result, b.result);
    let ret = SimpleLanguage::op_return(&mut stage, c.result);
    let block = stage
        .block()
        .stmt(a)
        .stmt(b)
        .stmt(c)
        .terminator(ret)
        .new();
    let body = stage.region().add_block(block).new();
    let fdef = SimpleLanguage::op_function(&mut stage, body);
    let f = stage
        .specialize()
        .staged_func(sf)
        .body(fdef)
        .new()
        .unwrap();

    // Render with narrow width
    let output = PrettyPrintExt::<SimpleLanguage>::render(&f, &stage)
        .config(Config::default().with_width(30))
        .globals(&gs)
        .to_string()
        .unwrap();
    insta::assert_snapshot!(output);
}
