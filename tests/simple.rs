use kirin::prelude::*;
use kirin_test_languages::{SimpleLanguage, SimpleType};

#[test]
fn test_block() {
    let mut gs: kirin_ir::InternTable<String, kirin_ir::GlobalSymbol> =
        kirin_ir::InternTable::default();
    let foo = gs.intern("foo".to_string());
    let mut stage: StageInfo<SimpleLanguage> = StageInfo::default();
    let staged_function = stage
        .staged_function()
        .name(foo)
        .signature(kirin_ir::Signature {
            params: vec![SimpleType::I64],
            ret: SimpleType::I64,
            constraints: (),
        })
        .new()
        .unwrap();

    let a = SimpleLanguage::op_constant(&mut stage, 1.2);
    let b = SimpleLanguage::op_constant(&mut stage, 3.4);
    let c = SimpleLanguage::op_add(&mut stage, a.result, b.result);
    let block_arg_x = stage.block_argument(0);
    let d = SimpleLanguage::op_add(&mut stage, c.result, block_arg_x);
    let ret = SimpleLanguage::op_return(&mut stage, d.result);

    let block_a: Block = stage
        .block()
        .argument(SimpleType::I64)
        .argument_with_name("y", SimpleType::F64)
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
    let f = stage
        .specialize()
        .f(staged_function)
        .body(fdef)
        .new()
        .unwrap();

    // Pretty print the function using the Document method
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

// ============================================================================
// Roundtrip Tests
// ============================================================================

use kirin::parsers::{EmitContext, EmitIR, parse_ast};
use kirin::pretty::Config;

/// Test roundtrip: parse -> emit -> print should produce output matching input.
#[test]
fn test_roundtrip_add() {
    let mut stage: StageInfo<SimpleLanguage> = StageInfo::default();

    // Create operand SSAs with types
    let ssa_a = stage
        .ssa()
        .name("a".to_string())
        .ty(SimpleType::I64)
        .kind(SSAKind::Test)
        .new();
    let ssa_b = stage
        .ssa()
        .name("b".to_string())
        .ty(SimpleType::I64)
        .kind(SSAKind::Test)
        .new();

    // Parse - type annotation in input
    let input = "%res = add %a, %b -> f64";
    let ast = parse_ast::<SimpleLanguage>(input).expect("parse failed");

    // Emit to get the dialect variant
    let mut emit_ctx = EmitContext::new(&mut stage);
    emit_ctx.register_ssa("a".to_string(), ssa_a);
    emit_ctx.register_ssa("b".to_string(), ssa_b);

    let statement = ast.emit(&mut emit_ctx);
    let stmt_info = statement.get_info(&stage).expect("stmt should exist");
    let dialect = stmt_info.definition();

    // Verify the result has the correct type by checking the SSA
    if let SimpleLanguage::Add(_, _, res) = dialect {
        let res_ssa: kirin_ir::SSAValue = (*res).into();
        let res_info = res_ssa.get_info(&stage).expect("result SSA should exist");
        assert_eq!(
            res_info.ty(),
            &SimpleType::F64,
            "Result type should be F64"
        );
    }

    // Pretty print directly using the trait
    let config = Config::default();
    let doc = Document::new(config, &stage);
    let arena_doc = dialect.pretty_print(&doc);
    let mut output = String::new();
    arena_doc
        .render_fmt(80, &mut output)
        .expect("render failed");

    // Compare (trim whitespace)
    assert_eq!(output.trim(), input);
}

/// Test roundtrip for constant instruction.
#[test]
fn test_roundtrip_constant() {
    use kirin::pretty::PrettyPrint as _;

    let mut stage: StageInfo<SimpleLanguage> = StageInfo::default();

    // Parse - type annotation in input
    let input = "%x = constant 42 -> f64";
    let ast = parse_ast::<SimpleLanguage>(input).expect("parse failed");

    // Emit
    let mut emit_ctx = EmitContext::new(&mut stage);
    let statement = ast.emit(&mut emit_ctx);
    let stmt_info = statement.get_info(&stage).expect("stmt should exist");
    let dialect = stmt_info.definition();

    // Verify the result has the correct type
    if let SimpleLanguage::Constant(_, res) = dialect {
        let res_ssa: kirin_ir::SSAValue = (*res).into();
        let res_info = res_ssa.get_info(&stage).expect("result SSA should exist");
        assert_eq!(
            res_info.ty(),
            &SimpleType::F64,
            "Result type should be F64"
        );
    }

    // Pretty print
    let config = Config::default();
    let doc = Document::new(config, &stage);
    let arena_doc = dialect.pretty_print(&doc);
    let mut output = String::new();
    arena_doc
        .render_fmt(80, &mut output)
        .expect("render failed");

    // Compare
    assert_eq!(output.trim(), input);
}

/// Test roundtrip for return instruction.
#[test]
fn test_roundtrip_return() {
    use kirin::pretty::PrettyPrint as _;

    let mut stage: StageInfo<SimpleLanguage> = StageInfo::default();

    // Create operand SSA
    let ssa_v = stage
        .ssa()
        .name("v".to_string())
        .ty(SimpleType::I64)
        .kind(SSAKind::Test)
        .new();

    // Parse
    let input = "return %v";
    let ast = parse_ast::<SimpleLanguage>(input).expect("parse failed");

    // Emit
    let mut emit_ctx = EmitContext::new(&mut stage);
    emit_ctx.register_ssa("v".to_string(), ssa_v);

    let statement = ast.emit(&mut emit_ctx);
    let stmt_info = statement.get_info(&stage).expect("stmt should exist");
    let dialect = stmt_info.definition();

    // Pretty print
    let config = Config::default();
    let doc = Document::new(config, &stage);
    let arena_doc = dialect.pretty_print(&doc);
    let mut output = String::new();
    arena_doc
        .render_fmt(80, &mut output)
        .expect("render failed");

    // Compare
    assert_eq!(output.trim(), input);
}

/// Strip trailing whitespace in each line of the input string.
pub fn strip_trailing_whitespace(s: &str) -> String {
    if s.is_empty() {
        return "\n".to_string();
    }
    let mut res = String::with_capacity(s.len());
    for line in s.lines() {
        res.push_str(line.trim_end());
        res.push('\n');
    }
    res
}

/// Test roundtrip for a full function with region containing multiple blocks and statements.
///
/// Note: This test verifies that parsing and emitting functions with regions works correctly.
/// The exact output format may differ from input due to Block/Region pretty printing details
/// (e.g., block names, result alignment), but the core structure is preserved.
#[test]
fn test_roundtrip_function() {
    let mut stage: StageInfo<SimpleLanguage> = StageInfo::default();

    // Parse a function with a region containing a block with multiple statements
    let input = r#"%f = function {
    ^entry(%x: f64) {
        %y = add %x, %x -> f64;
        %z = constant 42 -> f64;
        %w = add %y, %z -> f64;
        return %w;
    }
}"#;

    let ast = parse_ast::<SimpleLanguage>(input).expect("parse failed");

    // Emit to IR
    let mut emit_ctx = EmitContext::new(&mut stage);
    let statement = ast.emit(&mut emit_ctx);

    // Pretty print using Document method
    let doc = Document::new(Config::default(), &stage);
    let arena_doc = doc.print_statement(&statement);
    let max_width = doc.config().max_width;
    let mut buf = String::new();
    arena_doc
        .render_fmt(max_width, &mut buf)
        .expect("render failed");

    // Verify key structural elements are present
    assert!(
        buf.contains("%f = function"),
        "Should have function result name"
    );
    assert!(buf.contains("add"), "Should have add instruction");
    assert!(
        buf.contains("constant 42"),
        "Should have constant instruction"
    );
    assert!(buf.contains("return"), "Should have return instruction");
}

/// Test roundtrip for a function with multiple blocks in the region.
///
/// Note: This test verifies that parsing and emitting functions with multiple blocks works.
/// The exact output format may differ from input due to Block/Region pretty printing details.
#[test]
fn test_roundtrip_function_multiple_blocks() {
    let mut stage: StageInfo<SimpleLanguage> = StageInfo::default();

    // Parse a function with a region containing multiple blocks
    let input = r#"%f = function {
    ^entry(%x: f64) {
        %y = add %x, %x -> f64;
        return %y;
    }
    ^second(%a: f64) {
        %b = constant 100 -> f64;
        return %b;
    }
}"#;

    let ast = parse_ast::<SimpleLanguage>(input).expect("parse failed");

    // Emit to IR
    let mut emit_ctx = EmitContext::new(&mut stage);
    let statement = ast.emit(&mut emit_ctx);

    // Pretty print using Document method with 4-space indentation to match input
    let config = Config {
        tab_spaces: 4,
        ..Default::default()
    };
    let doc = Document::new(config, &stage);
    let arena_doc = doc.print_statement(&statement);
    let max_width = doc.config().max_width;
    let mut output = String::new();
    arena_doc
        .render_fmt(max_width, &mut output)
        .expect("render failed");
    println!("{}", output);
    // Note: output has a trailing newline from pretty printer
    assert_eq!(output.trim_end(), input);
}
