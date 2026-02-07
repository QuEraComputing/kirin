//! Integration and unit tests for pretty printing.

use kirin_ir::{Block, Dialect, GlobalSymbol, InternTable, Pipeline};
use kirin_test_utils::*;
use prettyless::DocAllocator;

use crate::{ArenaDoc, Config, Document, FunctionPrintExt, PrettyPrint, PrettyPrintExt};

// Implement PrettyPrint for the test dialect
impl PrettyPrint for SimpleLanguage {
    fn pretty_print<'a, L: Dialect + PrettyPrint>(&self, doc: &'a Document<'a, L>) -> ArenaDoc<'a>
    where
        L::Type: std::fmt::Display,
    {
        match self {
            SimpleLanguage::Add(lhs, rhs, _) => doc.text(format!("add {}, {}", *lhs, *rhs)),
            SimpleLanguage::Constant(value, _) => match value {
                Value::I64(v) => doc.text(format!("constant {}", v)),
                Value::F64(v) => doc.text(format!("constant {}", v)),
            },
            SimpleLanguage::Return(retval) => doc.text(format!("return {}", *retval)),
            SimpleLanguage::Function(region, _) => doc.print_region(region),
        }
    }
}

/// Helper to create a test context with a simple function.
fn create_test_function() -> (
    kirin_ir::Context<SimpleLanguage>,
    InternTable<String, GlobalSymbol>,
    kirin_ir::SpecializedFunction,
) {
    let mut gs: InternTable<String, GlobalSymbol> = InternTable::default();
    let test_func = gs.intern("test_func".to_string());
    let mut context: kirin_ir::Context<SimpleLanguage> = kirin_ir::Context::default();
    let staged_function = context
        .staged_function()
        .name(test_func)
        .signature(kirin_ir::Signature {
            params: vec![Int],
            ret: Int,
            constraints: (),
        })
        .new()
        .unwrap();

    let a = SimpleLanguage::op_constant(&mut context, 1.2);
    let b = SimpleLanguage::op_constant(&mut context, 3.4);
    let c = SimpleLanguage::op_add(&mut context, a.result, b.result);
    let block_arg_x = context.block_argument(0);
    let d = SimpleLanguage::op_add(&mut context, c.result, block_arg_x);
    let ret = SimpleLanguage::op_return(&mut context, d.result);

    let block_a: Block = context
        .block()
        .argument(Int)
        .argument_with_name("y", Float)
        .stmt(a)
        .stmt(b)
        .stmt(c)
        .stmt(d)
        .terminator(ret)
        .new();

    let ret = SimpleLanguage::op_return(&mut context, block_arg_x);
    let block_b = context.block().argument(Float).terminator(ret).new();

    let body = context.region().add_block(block_a).add_block(block_b).new();
    let fdef = SimpleLanguage::op_function(&mut context, body);
    let f = context
        .specialize()
        .f(staged_function)
        .body(fdef)
        .new()
        .unwrap();

    (context, gs, f)
}

// ============================================================================
// Snapshot tests
// ============================================================================

#[test]
fn test_block() {
    let (context, gs, f) = create_test_function();

    // Use the Document method API for printing IR nodes
    let doc = Document::with_global_symbols(Default::default(), &context, &gs);
    let arena_doc = doc.print_specialized_function(&f);
    let max_width = doc.config().max_width;
    let mut buf = String::new();
    arena_doc.render_fmt(max_width, &mut buf).unwrap();
    insta::assert_snapshot!(buf);
}

#[test]
fn test_render_specialized_function() {
    let (context, gs, f) = create_test_function();

    // Test the Document render method
    let mut doc = Document::with_global_symbols(Default::default(), &context, &gs);
    let output = doc.render(&f).unwrap();
    insta::assert_snapshot!(output);
}

#[test]
fn test_custom_width() {
    let (context, gs, f) = create_test_function();

    let config = Config::default().with_width(40);
    let mut doc = Document::with_global_symbols(config, &context, &gs);
    let output = doc.render(&f).unwrap();
    insta::assert_snapshot!(output);
}

// ============================================================================
// Unit tests for Document
// ============================================================================

#[test]
fn test_document_list_empty() {
    let context: kirin_ir::Context<SimpleLanguage> = kirin_ir::Context::default();
    let doc = Document::new(Default::default(), &context);

    let items: Vec<i32> = vec![];
    let result = doc.list(items.iter(), ", ", |i| doc.text(format!("{}", i)));
    let mut buf = String::new();
    result.render_fmt(80, &mut buf).unwrap();
    insta::assert_snapshot!(buf, @"");
}

#[test]
fn test_document_list_single() {
    let context: kirin_ir::Context<SimpleLanguage> = kirin_ir::Context::default();
    let doc = Document::new(Default::default(), &context);

    let items = vec![42];
    let result = doc.list(items.iter(), ", ", |i| doc.text(format!("{}", i)));
    let mut buf = String::new();
    result.render_fmt(80, &mut buf).unwrap();
    insta::assert_snapshot!(buf, @"42");
}

#[test]
fn test_document_list_multiple() {
    let context: kirin_ir::Context<SimpleLanguage> = kirin_ir::Context::default();
    let doc = Document::new(Default::default(), &context);

    let items = vec![1, 2, 3];
    let result = doc.list(items.iter(), ", ", |i| doc.text(format!("{}", i)));
    let mut buf = String::new();
    result.render_fmt(80, &mut buf).unwrap();
    insta::assert_snapshot!(buf, @"1, 2, 3");
}

#[test]
fn test_document_indent() {
    let context: kirin_ir::Context<SimpleLanguage> = kirin_ir::Context::default();
    let config = Config::default().with_tab_spaces(4);
    let doc = Document::new(config, &context);

    // Create indented content with line breaks
    let inner = doc.text("hello") + doc.line() + doc.text("world");
    let result = doc.indent(inner);
    let mut buf = String::new();
    result.render_fmt(5, &mut buf).unwrap(); // Force line break
    insta::assert_snapshot!(buf);
}

// ============================================================================
// Unit tests for individual PrettyPrint implementations
// ============================================================================

#[test]
fn test_constant_pretty_print() {
    let mut gs: InternTable<String, GlobalSymbol> = InternTable::default();
    let test_sym = gs.intern("test".to_string());
    let mut context: kirin_ir::Context<SimpleLanguage> = kirin_ir::Context::default();
    let _ = context.staged_function().name(test_sym).new().unwrap();

    let const_op = SimpleLanguage::op_constant(&mut context, 42i64);
    let doc = Document::new(Default::default(), &context);
    let arena_doc = doc.print_statement(&const_op.id);
    let mut buf = String::new();
    arena_doc.render_fmt(80, &mut buf).unwrap();
    insta::assert_snapshot!(buf, @"constant 42");
}

#[test]
fn test_add_pretty_print() {
    let mut gs: InternTable<String, GlobalSymbol> = InternTable::default();
    let test_sym = gs.intern("test".to_string());
    let mut context: kirin_ir::Context<SimpleLanguage> = kirin_ir::Context::default();
    let _ = context.staged_function().name(test_sym).new().unwrap();

    let a = SimpleLanguage::op_constant(&mut context, 1i64);
    let b = SimpleLanguage::op_constant(&mut context, 2i64);
    let add = SimpleLanguage::op_add(&mut context, a.result, b.result);

    let doc = Document::new(Default::default(), &context);
    let arena_doc = doc.print_statement(&add.id);
    let mut buf = String::new();
    arena_doc.render_fmt(80, &mut buf).unwrap();
    insta::assert_snapshot!(buf, @"add %0, %1");
}

// ============================================================================
// Write tests
// ============================================================================

#[test]
fn test_write_to_vec() {
    let (context, gs, f) = create_test_function();

    let mut doc = Document::with_global_symbols(Default::default(), &context, &gs);
    let output = doc.render(&f).unwrap();
    insta::assert_snapshot!(output);
}

#[test]
fn test_write_with_config() {
    let (context, gs, f) = create_test_function();

    let config = Config::default().with_width(40);
    let mut doc = Document::with_global_symbols(config, &context, &gs);
    let output = doc.render(&f).unwrap();
    insta::assert_snapshot!(output);
}

// ============================================================================
// GlobalSymbol PrettyPrint tests
// ============================================================================

#[test]
fn test_global_symbol_with_table() {
    let mut gs: InternTable<String, GlobalSymbol> = InternTable::default();
    let foo = gs.intern("foo".to_string());

    let context: kirin_ir::Context<SimpleLanguage> = kirin_ir::Context::default();
    let doc = Document::with_global_symbols(Default::default(), &context, &gs);
    let arena_doc = foo.pretty_print(&doc);
    let mut buf = String::new();
    arena_doc.render_fmt(80, &mut buf).unwrap();
    insta::assert_snapshot!(buf, @"@foo");
}

#[test]
fn test_global_symbol_without_table() {
    let mut gs: InternTable<String, GlobalSymbol> = InternTable::default();
    let foo = gs.intern("foo".to_string());

    let context: kirin_ir::Context<SimpleLanguage> = kirin_ir::Context::default();
    // Document without global symbols -- falls back to raw ID
    let doc = Document::new(Default::default(), &context);
    let arena_doc = foo.pretty_print(&doc);
    let mut buf = String::new();
    arena_doc.render_fmt(80, &mut buf).unwrap();
    insta::assert_snapshot!(buf, @"@<global:0>");
}

// ============================================================================
// sprint_with_globals tests
// ============================================================================

#[test]
fn test_sprint_with_globals() {
    let mut gs: InternTable<String, GlobalSymbol> = InternTable::default();
    let test_func = gs.intern("my_function".to_string());

    let mut context: kirin_ir::Context<SimpleLanguage> = kirin_ir::Context::default();
    let staged_function = context
        .staged_function()
        .name(test_func)
        .signature(kirin_ir::Signature {
            params: vec![Int],
            ret: Int,
            constraints: (),
        })
        .new()
        .unwrap();

    let a = SimpleLanguage::op_constant(&mut context, 42i64);
    let ret = SimpleLanguage::op_return(&mut context, a.result);
    let block = context.block().stmt(a).terminator(ret).new();
    let body = context.region().add_block(block).new();
    let fdef = SimpleLanguage::op_function(&mut context, body);
    let _ = context
        .specialize()
        .f(staged_function)
        .body(fdef)
        .new()
        .unwrap();

    // sprint_with_globals should resolve the function name
    let output = staged_function.sprint_with_globals(&context, &gs);
    insta::assert_snapshot!(output);
}

// ============================================================================
// Pipeline printing tests
// ============================================================================

#[test]
fn test_pipeline_function_print() {
    let mut pipeline: Pipeline<kirin_ir::Context<SimpleLanguage>> = Pipeline::new();
    let func = pipeline.function().name("foo").new();

    // --- Stage A: a simple function with one constant ---
    let stage0_id = pipeline
        .add_stage()
        .stage(kirin_ir::Context::default())
        .name("A")
        .new();
    let sf0 = pipeline
        .staged_function()
        .func(func)
        .stage(stage0_id)
        .signature(kirin_ir::Signature {
            params: vec![Int],
            ret: Int,
            constraints: (),
        })
        .new()
        .unwrap();

    let ctx0 = pipeline.stage_mut(stage0_id).unwrap();
    let a = SimpleLanguage::op_constant(ctx0, 42i64);
    let ret = SimpleLanguage::op_return(ctx0, a.result);
    let block = ctx0.block().stmt(a).terminator(ret).new();
    let body = ctx0.region().add_block(block).new();
    let fdef = SimpleLanguage::op_function(ctx0, body);
    ctx0.specialize().f(sf0).body(fdef).new().unwrap();

    // --- Stage B: a different version with two constants ---
    let stage1_id = pipeline
        .add_stage()
        .stage(kirin_ir::Context::default())
        .name("B")
        .new();
    let sf1 = pipeline
        .staged_function()
        .func(func)
        .stage(stage1_id)
        .signature(kirin_ir::Signature {
            params: vec![Int],
            ret: Int,
            constraints: (),
        })
        .new()
        .unwrap();

    let ctx1 = pipeline.stage_mut(stage1_id).unwrap();
    let a = SimpleLanguage::op_constant(ctx1, 10i64);
    let b = SimpleLanguage::op_constant(ctx1, 20i64);
    let c = SimpleLanguage::op_add(ctx1, a.result, b.result);
    let ret = SimpleLanguage::op_return(ctx1, c.result);
    let block = ctx1.block().stmt(a).stmt(b).stmt(c).terminator(ret).new();
    let body = ctx1.region().add_block(block).new();
    let fdef = SimpleLanguage::op_function(ctx1, body);
    ctx1.specialize().f(sf1).body(fdef).new().unwrap();

    // Print the function across both stages
    let output = func.sprint(&pipeline);
    insta::assert_snapshot!(output);
}

#[test]
fn test_pipeline_unnamed_stage() {
    let mut pipeline: Pipeline<kirin_ir::Context<SimpleLanguage>> = Pipeline::new();
    let func = pipeline.function().name("bar").new();

    // --- Unnamed stage (no .name() call) ---
    let stage_id = pipeline
        .add_stage()
        .stage(kirin_ir::Context::default())
        .new();
    let sf = pipeline
        .staged_function()
        .func(func)
        .stage(stage_id)
        .signature(kirin_ir::Signature {
            params: vec![Int, Float],
            ret: Int,
            constraints: (),
        })
        .new()
        .unwrap();

    let ctx = pipeline.stage_mut(stage_id).unwrap();
    let a = SimpleLanguage::op_constant(ctx, 7i64);
    let ret = SimpleLanguage::op_return(ctx, a.result);
    let block = ctx.block().stmt(a).terminator(ret).new();
    let body = ctx.region().add_block(block).new();
    let fdef = SimpleLanguage::op_function(ctx, body);
    ctx.specialize().f(sf).body(fdef).new().unwrap();

    // Should fall back to numeric index: "stage 0"
    let output = func.sprint(&pipeline);
    insta::assert_snapshot!(output);
}

#[test]
fn test_pipeline_staged_function_no_specialization() {
    let mut pipeline: Pipeline<kirin_ir::Context<SimpleLanguage>> = Pipeline::new();
    let func = pipeline.function().name("extern_fn").new();

    // Stage with a named stage but no specialization (declaration-only)
    let stage_id = pipeline
        .add_stage()
        .stage(kirin_ir::Context::default())
        .name("host")
        .new();
    let _sf = pipeline
        .staged_function()
        .func(func)
        .stage(stage_id)
        .signature(kirin_ir::Signature {
            params: vec![Int],
            ret: Float,
            constraints: (),
        })
        .new()
        .unwrap();

    // No specialize() call — staged function has no body / specializations
    let output = func.sprint(&pipeline);
    insta::assert_snapshot!(output);
}
