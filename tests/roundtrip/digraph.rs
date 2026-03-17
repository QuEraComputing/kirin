use kirin::prelude::*;
use kirin_test_languages::SimpleType;
use kirin_test_utils::roundtrip;

/// A simple digraph language for roundtrip testing.
///
/// Uses SimpleType as the type lattice. Inner statements are add/constant.
#[derive(Debug, Clone, PartialEq, Dialect, HasParser, PrettyPrint)]
#[kirin(builders, type = SimpleType, crate = kirin::ir)]
#[chumsky(crate = kirin::parsers)]
enum DiGraphLanguage {
    #[chumsky(format = "{2:name} = {.add} {0}, {1}")]
    Add(
        SSAValue,
        SSAValue,
        #[kirin(type = SimpleType::F64)] ResultValue,
    ),
    #[chumsky(format = "{1:name} = {.constant} {0}")]
    Constant(
        #[kirin(into)] kirin_test_languages::Value,
        #[kirin(type = SimpleType::F64)] ResultValue,
    ),
    #[chumsky(format = "{1:name} = {.graph_func} {0}")]
    GraphFunc(DiGraph, #[kirin(type = SimpleType::F64)] ResultValue),
}

// --- Statement-level roundtrip tests ---

#[test]
fn test_digraph_add_roundtrip() {
    roundtrip::assert_statement_roundtrip::<DiGraphLanguage>(
        "%r = add %a, %b",
        &[("a", SimpleType::F64), ("b", SimpleType::F64)],
    );
}

#[test]
fn test_digraph_constant_roundtrip() {
    roundtrip::assert_statement_roundtrip::<DiGraphLanguage>(
        "%c = constant 3.14",
        &[],
    );
}

#[test]
fn test_digraph_body_parse_and_render() {
    // Parse a digraph body and verify it renders correctly
    let input = "%out = graph_func digraph ^dg0(%p0: f64) { %c = constant 1; %r = add %p0, %c; yield %r; }";
    let (stage, stmt) = roundtrip::emit_statement::<DiGraphLanguage>(input, &[]);
    let rendered = roundtrip::render_statement::<DiGraphLanguage>(&stage, stmt);

    // Verify key structural elements are present
    assert!(rendered.contains("digraph ^dg0"));
    assert!(rendered.contains("constant 1"));
    assert!(rendered.contains("add"));
    assert!(rendered.contains("yield %r"));

    // The builder reorders nodes (topological sort), so the rendered output
    // may have forward SSA references (e.g. `add %p0, %c` before `%c = constant 1`).
    // Reparsing this requires relaxed-dominance support in the emitter.
    // This is tracked as a known limitation — verify it fails as expected.
    let reparse_result = std::panic::catch_unwind(|| {
        roundtrip::emit_statement::<DiGraphLanguage>(rendered.trim(), &[])
    });
    assert!(
        reparse_result.is_err(),
        "multi-node digraph reparse should fail due to forward SSA references (relaxed dominance not yet supported)"
    );
}

#[test]
fn test_digraph_single_node_roundtrip() {
    // A digraph with a single node has no forward references
    let input = "%out = graph_func digraph ^dg0(%p0: f64) { %r = add %p0, %p0; yield %r; }";
    let (stage, stmt) = roundtrip::emit_statement::<DiGraphLanguage>(input, &[]);
    let rendered = roundtrip::render_statement::<DiGraphLanguage>(&stage, stmt);

    let (stage2, stmt2) = roundtrip::emit_statement::<DiGraphLanguage>(rendered.trim(), &[]);
    let rendered2 = roundtrip::render_statement::<DiGraphLanguage>(&stage2, stmt2);
    assert_eq!(rendered.trim(), rendered2.trim(), "roundtrip should be stable");
}

#[test]
fn test_digraph_with_captures_roundtrip() {
    // Captures are always available, no forward reference issue
    let input = "%out = graph_func digraph ^dg0(%p0: f64) capture(%theta: f64) { %r = add %p0, %theta; yield %r; }";
    let (stage, stmt) = roundtrip::emit_statement::<DiGraphLanguage>(input, &[]);
    let rendered = roundtrip::render_statement::<DiGraphLanguage>(&stage, stmt);

    assert!(rendered.contains("capture(%theta: f64)"));

    let (stage2, stmt2) = roundtrip::emit_statement::<DiGraphLanguage>(rendered.trim(), &[]);
    let rendered2 = roundtrip::render_statement::<DiGraphLanguage>(&stage2, stmt2);
    assert_eq!(rendered.trim(), rendered2.trim());
}

#[test]
fn test_digraph_empty_body_roundtrip() {
    let input = "%out = graph_func digraph ^dg0(%p0: f64) { yield %p0; }";
    let (stage, stmt) = roundtrip::emit_statement::<DiGraphLanguage>(input, &[]);
    let rendered = roundtrip::render_statement::<DiGraphLanguage>(&stage, stmt);

    let (stage2, stmt2) = roundtrip::emit_statement::<DiGraphLanguage>(rendered.trim(), &[]);
    let rendered2 = roundtrip::render_statement::<DiGraphLanguage>(&stage2, stmt2);
    assert_eq!(rendered.trim(), rendered2.trim());
}
