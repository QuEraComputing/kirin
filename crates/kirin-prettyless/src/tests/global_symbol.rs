#[test]
fn test_global_symbol_with_table() {
    let mut gs: InternTable<String, GlobalSymbol> = InternTable::default();
    let foo = gs.intern("foo".to_string());

    let stage: kirin_ir::StageInfo<SimpleLanguage> = kirin_ir::StageInfo::default();
    let doc = Document::with_global_symbols(Default::default(), &stage, &gs);
    let arena_doc = foo.pretty_print(&doc);
    let mut buf = String::new();
    arena_doc.render_fmt(80, &mut buf).unwrap();
    insta::assert_snapshot!(buf, @"@foo");
}

#[test]
fn test_global_symbol_without_table() {
    let mut gs: InternTable<String, GlobalSymbol> = InternTable::default();
    let foo = gs.intern("foo".to_string());

    let stage: kirin_ir::StageInfo<SimpleLanguage> = kirin_ir::StageInfo::default();
    // Document without global symbols -- falls back to raw ID
    let doc = Document::new(Default::default(), &stage);
    let arena_doc = foo.pretty_print(&doc);
    let mut buf = String::new();
    arena_doc.render_fmt(80, &mut buf).unwrap();
    insta::assert_snapshot!(buf, @"@<global:0>");
}
