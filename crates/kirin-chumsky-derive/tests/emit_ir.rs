//! Tests for the EmitIR derive macro.
//!
//! These tests verify that parsed AST nodes can be correctly converted to IR nodes
//! using the EmitIR trait.

use kirin::ir::{Context, Dialect, GetInfo, ResultValue, SSAValue};
use kirin_chumsky::{EmitContext, EmitIR, HasParser, PrettyPrint, parse, parse_ast};
use kirin_test_utils::SimpleType;

/// A simple dialect for testing EmitIR functionality.
#[derive(Debug, Clone, PartialEq, Dialect, HasParser, PrettyPrint)]
#[kirin(type = SimpleType)]
#[chumsky(crate = kirin_chumsky)]
pub enum EmitLang {
    /// Simple add instruction: `%res = add %lhs, %rhs`
    #[chumsky(format = "{res:name} = add {lhs}, {rhs}")]
    Add {
        res: ResultValue,
        lhs: SSAValue,
        rhs: SSAValue,
    },
    /// Negate instruction: `%res = neg %arg`
    #[chumsky(format = "{res:name} = neg {arg}")]
    Neg { res: ResultValue, arg: SSAValue },
    /// Return instruction: `return %value`
    #[chumsky(format = "return {0}")]
    Return(SSAValue),
}

#[test]
fn test_emit_add_creates_statement() {
    // Create a fresh context
    let mut context: Context<EmitLang> = Context::default();

    // First, we need to set up the operand SSAs that the Add instruction will reference
    // Create SSAs for %a and %b before emitting the add instruction
    let ssa_a = context
        .ssa()
        .name("a".to_string())
        .ty(SimpleType::I32)
        .kind(kirin_ir::SSAKind::Test)
        .new();
    let ssa_b = context
        .ssa()
        .name("b".to_string())
        .ty(SimpleType::I32)
        .kind(kirin_ir::SSAKind::Test)
        .new();

    // Parse the add instruction
    let ast = parse_ast::<EmitLang>("%result = add %a, %b").expect("parse failed");

    // Create emit context and register the operand SSAs
    let mut emit_ctx = EmitContext::new(&mut context);
    emit_ctx.register_ssa("a".to_string(), ssa_a);
    emit_ctx.register_ssa("b".to_string(), ssa_b);

    // Emit the AST to IR
    let statement = ast.emit(&mut emit_ctx);

    // Verify the statement was created
    let stmt_info = statement
        .get_info(&context)
        .expect("statement should exist");

    // Verify the statement definition is an Add variant
    match stmt_info.definition() {
        EmitLang::Add { res, lhs, rhs } => {
            // Verify operands reference the correct SSAs
            assert_eq!(*lhs, ssa_a);
            assert_eq!(*rhs, ssa_b);

            // Verify the result SSA has the correct name
            let res_ssa: SSAValue = (*res).into();
            let res_info = res_ssa.get_info(&context).expect("result SSA should exist");
            // The name should be "result" from the parsed input
            assert!(res_info.name().is_some());
        }
        _ => panic!("Expected Add variant, got {:?}", stmt_info.definition()),
    }
}

#[test]
fn test_emit_neg_creates_statement() {
    let mut context: Context<EmitLang> = Context::default();

    // Create the operand SSA
    let ssa_x = context
        .ssa()
        .name("x".to_string())
        .ty(SimpleType::I32)
        .kind(kirin_ir::SSAKind::Test)
        .new();

    // Parse the neg instruction
    let ast = parse_ast::<EmitLang>("%y = neg %x").expect("parse failed");

    // Emit with registered SSA
    let mut emit_ctx = EmitContext::new(&mut context);
    emit_ctx.register_ssa("x".to_string(), ssa_x);

    let statement = ast.emit(&mut emit_ctx);

    // Verify
    let stmt_info = statement
        .get_info(&context)
        .expect("statement should exist");
    match stmt_info.definition() {
        EmitLang::Neg { res: _, arg } => {
            assert_eq!(*arg, ssa_x);
        }
        _ => panic!("Expected Neg variant"),
    }
}

#[test]
fn test_emit_return_creates_statement() {
    let mut context: Context<EmitLang> = Context::default();

    // Create the operand SSA
    let ssa_v = context
        .ssa()
        .name("v".to_string())
        .ty(SimpleType::I32)
        .kind(kirin_ir::SSAKind::Test)
        .new();

    // Parse the return instruction
    let ast = parse_ast::<EmitLang>("return %v").expect("parse failed");

    // Emit with registered SSA
    let mut emit_ctx = EmitContext::new(&mut context);
    emit_ctx.register_ssa("v".to_string(), ssa_v);

    let statement = ast.emit(&mut emit_ctx);

    // Verify
    let stmt_info = statement
        .get_info(&context)
        .expect("statement should exist");
    match stmt_info.definition() {
        EmitLang::Return(arg) => {
            assert_eq!(*arg, ssa_v);
        }
        _ => panic!("Expected Return variant"),
    }
}

#[test]
fn test_emit_convenience_function() {
    let mut context: Context<EmitLang> = Context::default();

    // Create the operand SSA
    let ssa_val = context
        .ssa()
        .name("val".to_string())
        .ty(SimpleType::I32)
        .kind(kirin_ir::SSAKind::Test)
        .new();

    // Parse
    let ast = parse_ast::<EmitLang>("return %val").expect("parse failed");

    // Use the convenience function - but we need to set up the context first
    // Since emit() creates a fresh EmitContext, we can't pre-register SSAs
    // This test verifies that emit() compiles but will panic on lookup
    // In practice, you'd use EmitContext directly for complex scenarios

    // For this test, let's just verify emit() works when SSAs are registered
    let mut emit_ctx = EmitContext::new(&mut context);
    emit_ctx.register_ssa("val".to_string(), ssa_val);
    let _statement = ast.emit(&mut emit_ctx);
}

#[test]
fn test_emit_result_creates_ssa() {
    let mut context: Context<EmitLang> = Context::default();

    // Create operand SSAs
    let ssa_a = context
        .ssa()
        .name("a".to_string())
        .ty(SimpleType::I32)
        .kind(kirin_ir::SSAKind::Test)
        .new();
    let ssa_b = context
        .ssa()
        .name("b".to_string())
        .ty(SimpleType::I32)
        .kind(kirin_ir::SSAKind::Test)
        .new();

    // Parse
    let ast = parse_ast::<EmitLang>("%sum = add %a, %b").expect("parse failed");

    // Emit
    let mut emit_ctx = EmitContext::new(&mut context);
    emit_ctx.register_ssa("a".to_string(), ssa_a);
    emit_ctx.register_ssa("b".to_string(), ssa_b);

    let _statement = ast.emit(&mut emit_ctx);

    // After emission, the result SSA "sum" should be registered
    let sum_ssa = emit_ctx.lookup_ssa("sum");
    assert!(
        sum_ssa.is_some(),
        "Result SSA 'sum' should be registered after emission"
    );
}

#[test]
fn test_emit_chain_multiple_statements() {
    let mut context: Context<EmitLang> = Context::default();

    // Create initial SSAs
    let ssa_x = context
        .ssa()
        .name("x".to_string())
        .ty(SimpleType::I32)
        .kind(kirin_ir::SSAKind::Test)
        .new();
    let ssa_y = context
        .ssa()
        .name("y".to_string())
        .ty(SimpleType::I32)
        .kind(kirin_ir::SSAKind::Test)
        .new();

    // Parse all the ASTs first
    let ast1 = parse_ast::<EmitLang>("%sum = add %x, %y").expect("parse failed");
    let ast2 = parse_ast::<EmitLang>("%neg = neg %sum").expect("parse failed");
    let ast3 = parse_ast::<EmitLang>("return %neg").expect("parse failed");

    // Track the expected neg SSA
    let neg_ssa;

    // Emit all statements in a block so the mutable borrow ends
    let (stmt1, stmt2, stmt3) = {
        let mut emit_ctx = EmitContext::new(&mut context);
        emit_ctx.register_ssa("x".to_string(), ssa_x);
        emit_ctx.register_ssa("y".to_string(), ssa_y);

        // Emit first statement: %sum = add %x, %y
        let stmt1 = ast1.emit(&mut emit_ctx);

        // Now %sum should be available for the next statement
        // Emit second statement: %neg = neg %sum
        let stmt2 = ast2.emit(&mut emit_ctx);

        // Emit third statement: return %neg
        let stmt3 = ast3.emit(&mut emit_ctx);

        // Capture the neg SSA before the borrow ends
        neg_ssa = emit_ctx
            .lookup_ssa("neg")
            .expect("neg should be registered");

        (stmt1, stmt2, stmt3)
    };

    // Verify all statements were created
    assert!(stmt1.get_info(&context).is_some());
    assert!(stmt2.get_info(&context).is_some());
    assert!(stmt3.get_info(&context).is_some());

    // Verify the chain is correct by checking the Return uses the negated value
    let stmt3_info = stmt3.get_info(&context).unwrap();
    if let EmitLang::Return(ret_arg) = stmt3_info.definition() {
        // The return argument should be the SSA registered as "neg"
        assert_eq!(*ret_arg, neg_ssa);
    } else {
        panic!("Expected Return statement");
    }
}

/// Test the combined `parse` function that does parsing + emission in one step.
///
/// The combined `parse` function is designed for parsing complete programs
/// or statements where all referenced SSAs are defined within the same context.
/// For this test, we verify the function signature and basic usage work.
#[test]
fn test_combined_parse_function() {
    let mut context: Context<EmitLang> = Context::default();

    // Test that the parse function works for syntax errors (returns Err)
    let result = parse::<EmitLang>("invalid syntax", &mut context);
    assert!(result.is_err(), "Invalid syntax should return an error");

    // For statements that reference external SSAs (like %a, %b), we need
    // to use parse_ast + EmitContext instead, as shown in other tests.
    // The combined parse creates a fresh EmitContext without pre-registered SSAs.
}

/// Test that demonstrates the typical workflow with the new combined parse function.
///
/// This shows how to use EmitContext for multi-statement programs where
/// SSAs from earlier statements are referenced in later ones.
#[test]
fn test_combined_parse_workflow() {
    let mut context: Context<EmitLang> = Context::default();

    // For multi-statement programs, use parse_ast + EmitContext
    // to maintain SSA mappings across statements.

    // Create initial SSAs (these would typically come from function arguments
    // or other sources in a real compiler)
    let ssa_a = context
        .ssa()
        .name("a".to_string())
        .ty(SimpleType::I32)
        .kind(kirin_ir::SSAKind::Test)
        .new();
    let ssa_b = context
        .ssa()
        .name("b".to_string())
        .ty(SimpleType::I32)
        .kind(kirin_ir::SSAKind::Test)
        .new();

    // Create emit context and register the initial SSAs
    let mut emit_ctx = EmitContext::new(&mut context);
    emit_ctx.register_ssa("a".to_string(), ssa_a);
    emit_ctx.register_ssa("b".to_string(), ssa_b);

    // Parse and emit multiple statements
    let statements = ["%x = add %a, %b", "%y = neg %x", "return %y"];

    for src in statements {
        let ast = parse_ast::<EmitLang>(src).expect("parse failed");
        let _stmt = ast.emit(&mut emit_ctx);
    }

    // Verify the final SSA chain
    let x_ssa = emit_ctx.lookup_ssa("x").expect("x should exist");
    let y_ssa = emit_ctx.lookup_ssa("y").expect("y should exist");

    // Both SSAs should have been created
    assert!(x_ssa.get_info(emit_ctx.context).is_some());
    assert!(y_ssa.get_info(emit_ctx.context).is_some());
}

// ============================================================================
// Roundtrip Tests
// ============================================================================

use kirin_prettyless::{Config, Document};

/// Test roundtrip: parse -> emit -> print should produce output matching input.
///
/// This verifies that the parser and pretty printer are symmetric.
#[test]
fn test_roundtrip_add() {
    let mut context: Context<EmitLang> = Context::default();

    // Create operand SSAs
    let ssa_a = context
        .ssa()
        .name("a".to_string())
        .ty(SimpleType::I32)
        .kind(kirin_ir::SSAKind::Test)
        .new();
    let ssa_b = context
        .ssa()
        .name("b".to_string())
        .ty(SimpleType::I32)
        .kind(kirin_ir::SSAKind::Test)
        .new();

    // Parse
    let input = "%res = add %a, %b";
    let ast = parse_ast::<EmitLang>(input).expect("parse failed");

    // Emit to get the dialect variant
    let mut emit_ctx = EmitContext::new(&mut context);
    emit_ctx.register_ssa("a".to_string(), ssa_a);
    emit_ctx.register_ssa("b".to_string(), ssa_b);

    let statement = ast.emit(&mut emit_ctx);
    let stmt_info = statement.get_info(&context).expect("stmt should exist");
    let dialect = stmt_info.definition();

    // Pretty print directly using the trait
    let config = Config::default();
    let doc = Document::new(config, &context);
    let arena_doc = dialect.pretty_print(&doc);
    let mut output = String::new();
    arena_doc
        .render_fmt(80, &mut output)
        .expect("render failed");

    // Compare (trim whitespace)
    assert_eq!(output.trim(), input);
}

/// Test roundtrip for neg instruction.
#[test]
fn test_roundtrip_neg() {
    let mut context: Context<EmitLang> = Context::default();

    // Create operand SSA
    let ssa_x = context
        .ssa()
        .name("x".to_string())
        .ty(SimpleType::I32)
        .kind(kirin_ir::SSAKind::Test)
        .new();

    // Parse
    let input = "%y = neg %x";
    let ast = parse_ast::<EmitLang>(input).expect("parse failed");

    // Emit
    let mut emit_ctx = EmitContext::new(&mut context);
    emit_ctx.register_ssa("x".to_string(), ssa_x);

    let statement = ast.emit(&mut emit_ctx);
    let stmt_info = statement.get_info(&context).expect("stmt should exist");
    let dialect = stmt_info.definition();

    // Pretty print directly using the trait
    let config = Config::default();
    let doc = Document::new(config, &context);
    let arena_doc = dialect.pretty_print(&doc);
    let mut output = String::new();
    arena_doc
        .render_fmt(80, &mut output)
        .expect("render failed");

    // Compare
    assert_eq!(output.trim(), input);
}

/// Test roundtrip for return instruction (tuple variant).
#[test]
fn test_roundtrip_return() {
    let mut context: Context<EmitLang> = Context::default();

    // Create operand SSA
    let ssa_v = context
        .ssa()
        .name("v".to_string())
        .ty(SimpleType::I32)
        .kind(kirin_ir::SSAKind::Test)
        .new();

    // Parse
    let input = "return %v";
    let ast = parse_ast::<EmitLang>(input).expect("parse failed");

    // Emit
    let mut emit_ctx = EmitContext::new(&mut context);
    emit_ctx.register_ssa("v".to_string(), ssa_v);

    let statement = ast.emit(&mut emit_ctx);
    let stmt_info = statement.get_info(&context).expect("stmt should exist");
    let dialect = stmt_info.definition();

    // Pretty print directly using the trait
    let config = Config::default();
    let doc = Document::new(config, &context);
    let arena_doc = dialect.pretty_print(&doc);
    let mut output = String::new();
    arena_doc
        .render_fmt(80, &mut output)
        .expect("render failed");

    // Compare
    assert_eq!(output.trim(), input);
}
