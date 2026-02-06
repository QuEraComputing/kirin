//! Integration and unit tests for pretty printing.

use kirin_ir::{Block, Dialect};
use kirin_test_utils::*;
use prettyless::DocAllocator;

use crate::{ArenaDoc, Config, Document, PrettyPrint};

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
            SimpleLanguage::Function(region, _) => doc.text("function ") + doc.print_region(region),
        }
    }
}

/// Helper to create a test context with a simple function.
fn create_test_function() -> (
    kirin_ir::Context<SimpleLanguage>,
    kirin_ir::SpecializedFunction,
) {
    let mut context: kirin_ir::Context<SimpleLanguage> = kirin_ir::Context::default();
    let staged_function = context
        .staged_function()
        .name("test_func")
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

    (context, f)
}

// ============================================================================
// Snapshot tests
// ============================================================================

#[test]
fn test_block() {
    let (context, f) = create_test_function();

    // Use the Document method API for printing IR nodes
    let doc = Document::new(Default::default(), &context);
    let arena_doc = doc.print_specialized_function(&f);
    let max_width = doc.config().max_width;
    let mut buf = String::new();
    arena_doc.render_fmt(max_width, &mut buf).unwrap();
    insta::assert_snapshot!(buf);
}

#[test]
fn test_render_specialized_function() {
    let (context, f) = create_test_function();

    // Test the Document render method
    let mut doc = Document::new(Default::default(), &context);
    let output = doc.render(&f).unwrap();
    insta::assert_snapshot!(output);
}

#[test]
fn test_custom_width() {
    let (context, f) = create_test_function();

    let config = Config::default().with_width(40);
    let mut doc = Document::new(config, &context);
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
    assert_eq!(buf, "");
}

#[test]
fn test_document_list_single() {
    let context: kirin_ir::Context<SimpleLanguage> = kirin_ir::Context::default();
    let doc = Document::new(Default::default(), &context);

    let items = vec![42];
    let result = doc.list(items.iter(), ", ", |i| doc.text(format!("{}", i)));
    let mut buf = String::new();
    result.render_fmt(80, &mut buf).unwrap();
    assert_eq!(buf, "42");
}

#[test]
fn test_document_list_multiple() {
    let context: kirin_ir::Context<SimpleLanguage> = kirin_ir::Context::default();
    let doc = Document::new(Default::default(), &context);

    let items = vec![1, 2, 3];
    let result = doc.list(items.iter(), ", ", |i| doc.text(format!("{}", i)));
    let mut buf = String::new();
    result.render_fmt(80, &mut buf).unwrap();
    assert_eq!(buf, "1, 2, 3");
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
    assert!(buf.contains("    hello") || buf.contains("    world"));
}

// ============================================================================
// Unit tests for individual PrettyPrint implementations
// ============================================================================

#[test]
fn test_constant_pretty_print() {
    let mut context: kirin_ir::Context<SimpleLanguage> = kirin_ir::Context::default();
    let _ = context.staged_function().name("test").new().unwrap();

    let const_op = SimpleLanguage::op_constant(&mut context, 42i64);
    let doc = Document::new(Default::default(), &context);
    let arena_doc = doc.print_statement(&const_op.id);
    let mut buf = String::new();
    arena_doc.render_fmt(80, &mut buf).unwrap();
    assert_eq!(buf, "constant 42");
}

#[test]
fn test_add_pretty_print() {
    let mut context: kirin_ir::Context<SimpleLanguage> = kirin_ir::Context::default();
    let _ = context.staged_function().name("test").new().unwrap();

    let a = SimpleLanguage::op_constant(&mut context, 1i64);
    let b = SimpleLanguage::op_constant(&mut context, 2i64);
    let add = SimpleLanguage::op_add(&mut context, a.result, b.result);

    let doc = Document::new(Default::default(), &context);
    let arena_doc = doc.print_statement(&add.id);
    let mut buf = String::new();
    arena_doc.render_fmt(80, &mut buf).unwrap();
    // The output should contain "add" and the SSA value references
    assert!(buf.starts_with("add "));
}

// ============================================================================
// Write tests
// ============================================================================

#[test]
fn test_write_to_vec() {
    let (context, f) = create_test_function();

    let mut doc = Document::new(Default::default(), &context);
    let output = doc.render(&f).unwrap();

    assert!(!output.is_empty());
    assert!(output.contains("function"));
}

#[test]
fn test_write_with_config() {
    let (context, f) = create_test_function();

    let config = Config::default().with_width(40);
    let mut doc = Document::new(config, &context);
    let output = doc.render(&f).unwrap();

    assert!(!output.is_empty());
}
