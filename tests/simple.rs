use kirin::prelude::*;
use kirin_test_languages::{SimpleLanguage, SimpleType};

#[test]
fn test_block() {
    let mut gs: kirin_ir::InternTable<String, kirin_ir::GlobalSymbol> =
        kirin_ir::InternTable::default();
    let foo = gs.intern("foo".to_string());
    let mut stage: BuilderStageInfo<SimpleLanguage> = BuilderStageInfo::default();
    let staged_function = stage
        .staged_function()
        .name(foo)
        .signature(
            kirin_ir::Signature::new()
                .takes([SimpleType::I64])
                .returns(SimpleType::I64),
        )
        .new()
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

    let block_b_arg = stage.block_argument().index(0);
    let ret = SimpleLanguage::op_return(&mut stage, block_b_arg);
    let block_b = stage
        .block()
        .argument(SimpleType::F64)
        .terminator(ret)
        .new();

    let body = stage.cfg().add_block(block_a).add_block(block_b).new();
    let fdef = SimpleLanguage::op_function(
        &mut stage,
        body,
        kirin_ir::Signature::new()
            .takes([SimpleType::I64])
            .returns(SimpleType::I64),
    );
    let f = stage
        .specialize()
        .staged_func(staged_function)
        .definition(fdef)
        .new()
        .unwrap();

    // Pretty print the function using the Document method
    let stage = stage.finalize().unwrap();
    let doc = Document::new(Default::default(), &stage);
    let arena_doc = doc.print_specialized_function(&f);
    let max_width = doc.config().max_width;
    let mut buf = String::new();
    arena_doc.render_fmt(max_width, &mut buf).unwrap();
    // Verify the output contains expected elements
    assert!(buf.contains("function"));
    assert!(buf.contains("constant"));
    assert!(buf.contains("add"));
    assert!(buf.contains("return"));
}

fn simple_sig(param: SimpleType, ret: SimpleType) -> kirin_ir::Signature<SimpleType> {
    kirin_ir::Signature::new().takes([param]).returns(ret)
}

/// A `function` definition with an empty body declaring `sig`.
fn simple_definition(
    stage: &mut BuilderStageInfo<SimpleLanguage>,
    sig: kirin_ir::Signature<SimpleType>,
) -> kirin_ir::Statement {
    let body = stage.cfg().new();
    SimpleLanguage::op_function(stage, body, sig).into()
}

/// `SimpleLanguage` opts in to `LatticeSemantics`, so a specialization may
/// narrow every position of its interface.
#[test]
fn lattice_dialect_accepts_a_specialization_narrower_than_its_interface() {
    let mut stage: BuilderStageInfo<SimpleLanguage> = BuilderStageInfo::default();
    let sf = stage
        .staged_function()
        .signature(simple_sig(SimpleType::Any, SimpleType::Any))
        .new()
        .unwrap();

    for sig in [
        simple_sig(SimpleType::I64, SimpleType::I64),
        simple_sig(SimpleType::F64, SimpleType::Any),
        simple_sig(SimpleType::Any, SimpleType::Bool),
    ] {
        let definition = simple_definition(&mut stage, sig);
        stage
            .specialize()
            .staged_func(sf)
            .definition(definition)
            .new()
            .expect("a narrower signature refines the interface");
    }
}

#[test]
fn lattice_dialect_rejects_a_specialization_outside_its_interface() {
    let mut stage: BuilderStageInfo<SimpleLanguage> = BuilderStageInfo::default();
    let sf = stage
        .staged_function()
        .signature(simple_sig(SimpleType::I64, SimpleType::I64))
        .new()
        .unwrap();

    for sig in [
        // A wider parameter, an unrelated parameter, and a wider return.
        simple_sig(SimpleType::Any, SimpleType::I64),
        simple_sig(SimpleType::F64, SimpleType::I64),
        simple_sig(SimpleType::I64, SimpleType::Any),
    ] {
        let definition = simple_definition(&mut stage, sig);
        let result = stage
            .specialize()
            .staged_func(sf)
            .definition(definition)
            .new();
        assert!(matches!(
            result,
            Err(kirin_ir::SpecializeError::NotARefinement { staged_function, definition: rejected })
                if staged_function == sf && rejected == definition
        ));
    }
}

/// Dispatch uses the dialect's semantics: the most specific applicable
/// specialization wins.
#[test]
fn lattice_dialect_dispatches_to_the_most_specific_specialization() {
    let mut stage: BuilderStageInfo<SimpleLanguage> = BuilderStageInfo::default();
    let sf = stage
        .staged_function()
        .signature(simple_sig(SimpleType::Any, SimpleType::F64))
        .new()
        .unwrap();

    let general = simple_definition(&mut stage, simple_sig(SimpleType::Any, SimpleType::F64));
    let general = stage
        .specialize()
        .staged_func(sf)
        .definition(general)
        .new()
        .unwrap();
    let specific = simple_definition(&mut stage, simple_sig(SimpleType::I64, SimpleType::F64));
    let specific = stage
        .specialize()
        .staged_func(sf)
        .definition(specific)
        .new()
        .unwrap();

    let stage = stage.finalize().unwrap();
    let info = sf.get_info(&stage).unwrap();
    let matched = |param| -> Vec<_> {
        info.all_matching(&stage, &simple_sig(param, SimpleType::F64))
            .into_iter()
            .map(|(spec, ())| spec.id())
            .collect()
    };
    assert_eq!(matched(SimpleType::I64), vec![specific]);
    assert_eq!(matched(SimpleType::Bool), vec![general]);
}
