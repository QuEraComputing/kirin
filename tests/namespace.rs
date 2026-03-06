use kirin::parsers::parse_ast;
use kirin::prelude::*;
use kirin_arith::{Arith, ArithType};
use kirin_test_languages::CompositeLanguage;

/// Test roundtrip: construct arith.add IR -> pretty print with namespace -> parse it back.
///
/// CompositeLanguage includes FunctionBody which causes E0275 (recursive trait overflow)
/// when calling EmitIR through the full parse+emit pipeline. Instead, we construct the IR
/// manually, pretty-print it (verifying the namespace prefix), and then parse the output
/// to verify the parser accepts the namespaced format.
#[test]
fn test_namespace_roundtrip_arith_add() {
    use kirin::pretty::{Config, PrettyPrint as _};

    let mut stage: StageInfo<CompositeLanguage> = StageInfo::default();

    // Create operand SSAs with types
    let ssa_a = stage
        .ssa()
        .name("a".to_string())
        .ty(ArithType::I64)
        .kind(SSAKind::Test)
        .new();
    let ssa_b = stage
        .ssa()
        .name("b".to_string())
        .ty(ArithType::I64)
        .kind(SSAKind::Test)
        .new();

    // Construct the arith.add statement via the generated builder
    let add_stmt = Arith::<ArithType>::op_add(&mut stage, ssa_a, ssa_b);

    // Pretty print
    let stmt_info = add_stmt.id.get_info(&stage).expect("stmt should exist");
    let dialect = stmt_info.definition();
    let config = Config::default();
    let doc = Document::new(config, &stage);
    let arena_doc = dialect.pretty_print(&doc);
    let mut output = String::new();
    arena_doc
        .render_fmt(80, &mut output)
        .expect("render failed");

    let printed = output.trim();

    // The namespace prefix "arith." should appear at the start (before the inner format)
    assert!(
        printed.starts_with("arith."),
        "expected output to start with 'arith.', got: {printed}"
    );

    // The inner format is: %<name> = add %a, %b -> i64
    // With namespace prefix: arith.%<name> = add %a, %b -> i64
    assert!(
        printed.contains("= add"),
        "expected '= add' in output, got: {printed}"
    );
    assert!(
        printed.ends_with("-> i64"),
        "expected output to end with '-> i64', got: {printed}"
    );

    // Now verify we can parse the printed output back (roundtrip)
    let _ast = parse_ast::<CompositeLanguage>(printed).expect("re-parse of printed output failed");
}

/// Test that parsing a namespaced statement works correctly.
#[test]
fn test_namespace_parse_arith_add() {
    let input = "arith.%res = add %a, %b -> i64";
    let _ast =
        parse_ast::<CompositeLanguage>(input).expect("parsing namespaced arith.add should succeed");
}
