#[test]
fn test_constant_pretty_print() {
    let mut gs: InternTable<String, GlobalSymbol> = InternTable::default();
    let test_sym = gs.intern("test".to_string());
    let mut stage: kirin_ir::StageInfo<SimpleLanguage> = kirin_ir::StageInfo::default();
    let _ = stage.staged_function().name(test_sym).new().unwrap();

    let const_op = SimpleLanguage::op_constant(&mut stage, 42i64);
    let doc = Document::new(Default::default(), &stage);
    let arena_doc = doc.print_statement(&const_op.id);
    let mut buf = String::new();
    arena_doc.render_fmt(80, &mut buf).unwrap();
    insta::assert_snapshot!(buf, @"constant 42");
}

#[test]
fn test_add_pretty_print() {
    let mut gs: InternTable<String, GlobalSymbol> = InternTable::default();
    let test_sym = gs.intern("test".to_string());
    let mut stage: kirin_ir::StageInfo<SimpleLanguage> = kirin_ir::StageInfo::default();
    let _ = stage.staged_function().name(test_sym).new().unwrap();

    let a = SimpleLanguage::op_constant(&mut stage, 1i64);
    let b = SimpleLanguage::op_constant(&mut stage, 2i64);
    let add = SimpleLanguage::op_add(&mut stage, a.result, b.result);

    let doc = Document::new(Default::default(), &stage);
    let arena_doc = doc.print_statement(&add.id);
    let mut buf = String::new();
    arena_doc.render_fmt(80, &mut buf).unwrap();
    insta::assert_snapshot!(buf, @"add %0, %1");
}
