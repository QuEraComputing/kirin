use kirin::prelude::*;
use kirin_test_languages::{SimpleLanguage, SimpleType};

#[test]
fn test_block() {
    let mut gs: kirin_ir::InternTable<String, kirin_ir::GlobalSymbol> =
        kirin_ir::InternTable::default();
    let foo = gs.intern("foo".to_string());
    let mut stage: BuilderStageInfo<SimpleLanguage> = BuilderStageInfo::default();
    let staged_function = stage
        .staged_function(
            Some(foo),
            Some(kirin_ir::Signature::new(
                vec![SimpleType::I64],
                SimpleType::I64,
                (),
            )),
            None,
            None,
        )
        .unwrap();

    let a = SimpleLanguage::op_constant(&mut stage, 1.2);
    let b = SimpleLanguage::op_constant(&mut stage, 3.4);
    let c = SimpleLanguage::op_add(&mut stage, a.result, b.result);
    let block_arg_x = stage.block_argument().index(0);
    let d = SimpleLanguage::op_add(&mut stage, c.result, block_arg_x);
    let ret = SimpleLanguage::op_return(&mut stage, d.result);

    let block_a: Block = stage
        .block()
        .argument(SimpleType::I64)
        .argument(SimpleType::F64)
        .arg_name("y")
        .stmt(a)
        .stmt(b)
        .stmt(c)
        .stmt(d)
        .terminator(ret)
        .new();

    let ret = SimpleLanguage::op_return(&mut stage, block_arg_x);
    let block_b = stage
        .block()
        .argument(SimpleType::F64)
        .terminator(ret)
        .new();

    let body = stage.region().add_block(block_a).add_block(block_b).new();
    let fdef = SimpleLanguage::op_function(&mut stage, body);
    let f = stage.specialize(staged_function, None, fdef, None).unwrap();

    // Pretty print the function using the Document method
    let stage = stage.finalize().unwrap();
    let doc = Document::new(Default::default(), &stage);
    let arena_doc = doc.print_specialized_function(&f);
    let max_width = doc.config().max_width;
    let mut buf = String::new();
    arena_doc.render_fmt(max_width, &mut buf).unwrap();
    println!("{}", buf);
    // Verify the output contains expected elements
    assert!(buf.contains("function"));
    assert!(buf.contains("constant"));
    assert!(buf.contains("add"));
    assert!(buf.contains("return"));
}
