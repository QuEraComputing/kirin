#[test]
fn test_sprint_with_globals() {
    let mut gs: InternTable<String, GlobalSymbol> = InternTable::default();
    let test_func = gs.intern("my_function".to_string());

    let mut stage: BuilderStageInfo<SimpleLanguage> = BuilderStageInfo::default();
    let staged_function = stage
        .staged_function()
        .name(test_func)
        .signature(kirin_ir::Signature::new(vec![SimpleType::I64], SimpleType::I64, ()))
        .new()
        .unwrap();

    let a = SimpleLanguage::op_constant(&mut stage, 42i64);
    let ret = SimpleLanguage::op_return(&mut stage, a.result);
    let block = stage.block().stmt(a).terminator(ret).new();
    let body = stage.region().add_block(block).new();
    let fdef = SimpleLanguage::op_function(&mut stage, body);
    let _ = stage
        .specialize()
        .staged_func(staged_function)
        .body(fdef)
        .new()
        .unwrap();

    // render with globals should resolve the function name
    let stage = stage.finalize().unwrap();
    let output = staged_function.render(&stage).globals(&gs).into_string().unwrap();
    insta::assert_snapshot!(output);
}
