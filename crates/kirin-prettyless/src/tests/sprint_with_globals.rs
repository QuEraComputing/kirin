#[test]
fn test_sprint_with_globals() {
    let mut gs: InternTable<String, GlobalSymbol> = InternTable::default();
    let test_func = gs.intern("my_function".to_string());

    let mut stage: kirin_ir::StageInfo<SimpleLanguage> = kirin_ir::StageInfo::default();
    let staged_function = stage
        .staged_function()
        .name(test_func)
        .signature(kirin_ir::Signature {
            params: vec![Int],
            ret: Int,
            constraints: (),
        })
        .new()
        .unwrap();

    let a = SimpleLanguage::op_constant(&mut stage, 42i64);
    let ret = SimpleLanguage::op_return(&mut stage, a.result);
    let block = stage.block().stmt(a).terminator(ret).new();
    let body = stage.region().add_block(block).new();
    let fdef = SimpleLanguage::op_function(&mut stage, body);
    let _ = stage
        .specialize()
        .f(staged_function)
        .body(fdef)
        .new()
        .unwrap();

    // sprint_with_globals should resolve the function name
    let output = staged_function.sprint_with_globals(&stage, &gs);
    insta::assert_snapshot!(output);
}
