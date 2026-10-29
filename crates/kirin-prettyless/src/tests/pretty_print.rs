#[test]
fn test_constant_pretty_print() {
    let mut gs: InternTable<String, GlobalSymbol> = InternTable::default();
    let test_sym = gs.intern("test".to_string());
    let mut stage: BuilderStageInfo<SimpleLanguage> = BuilderStageInfo::default();
    let _ = stage.staged_function().name(test_sym).new().unwrap();

    let const_op = SimpleLanguage::op_constant(&mut stage, 42i64);
    let stage = stage.finalize().unwrap();
    let doc = Document::new(Default::default(), &stage);
    let arena_doc = doc.print_statement(&const_op.id);
    let mut buf = String::new();
    arena_doc.render_fmt(80, &mut buf).unwrap();
    insta::assert_snapshot!(buf, @"%0 = constant 42");
}

#[test]
fn test_add_pretty_print() {
    let mut gs: InternTable<String, GlobalSymbol> = InternTable::default();
    let test_sym = gs.intern("test".to_string());
    let mut stage: BuilderStageInfo<SimpleLanguage> = BuilderStageInfo::default();
    let _ = stage.staged_function().name(test_sym).new().unwrap();

    let a = SimpleLanguage::op_constant(&mut stage, 1i64);
    let b = SimpleLanguage::op_constant(&mut stage, 2i64);
    let add = SimpleLanguage::op_add(&mut stage, a.result, b.result);

    let stage = stage.finalize().unwrap();
    let doc = Document::new(Default::default(), &stage);
    let arena_doc = doc.print_statement(&add.id);
    let mut buf = String::new();
    arena_doc.render_fmt(80, &mut buf).unwrap();
    insta::assert_snapshot!(buf, @"%2 = add %0, %1");
}

/// Abbreviating collapses the function's CFG body, and full printing
/// resumes once it returns.
#[test]
fn test_abbreviated_statement_collapses_nested_bodies() {
    let mut stage: BuilderStageInfo<SimpleLanguage> = BuilderStageInfo::default();
    let one = SimpleLanguage::op_constant(&mut stage, 1i64);
    let ret = SimpleLanguage::op_return(&mut stage, one.result);
    let block = stage.block().stmt(one).terminator(ret).new();
    let body = stage.cfg().add_block(block).new();
    let function = SimpleLanguage::op_function(&mut stage, body);
    let stage = stage.finalize().unwrap();

    let doc = Document::new(Default::default(), &stage);
    let mut abbreviated = String::new();
    doc.print_statement_abbreviated(&function.id)
        .render_fmt(80, &mut abbreviated)
        .unwrap();
    insta::assert_snapshot!(abbreviated, @"%1 = {...}");

    let mut full = String::new();
    doc.print_statement(&function.id)
        .render_fmt(80, &mut full)
        .unwrap();
    assert!(full.contains("return %0;"), "{full}");
}
