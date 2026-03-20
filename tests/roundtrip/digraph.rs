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
    #[chumsky(format = "$add {0}, {1}")]
    Add(
        SSAValue,
        SSAValue,
        #[kirin(type = SimpleType::F64)] ResultValue,
    ),
    #[chumsky(format = "$constant {0}")]
    Constant(
        #[kirin(into)] kirin_test_languages::Value,
        #[kirin(type = SimpleType::F64)] ResultValue,
    ),
    #[chumsky(format = "$graph_func {0}")]
    GraphFunc(DiGraph, #[kirin(type = SimpleType::F64)] ResultValue),
}

/// A digraph language variant using body projections.
///
/// Instead of `digraph ^name(...) { ... }`, this uses projected format:
/// `projected_func (%port: Type) { stmt; yield %v; }`
#[derive(Debug, Clone, PartialEq, Dialect, HasParser, PrettyPrint)]
#[kirin(builders, type = SimpleType, crate = kirin::ir)]
#[chumsky(crate = kirin::parsers)]
enum ProjectedDigraphLanguage {
    #[chumsky(format = "$add {0}, {1}")]
    Add(
        SSAValue,
        SSAValue,
        #[kirin(type = SimpleType::F64)] ResultValue,
    ),
    #[chumsky(format = "$constant {0}")]
    Constant(
        #[kirin(into)] kirin_test_languages::Value,
        #[kirin(type = SimpleType::F64)] ResultValue,
    ),
    #[chumsky(format = "$projected_func ({0:ports}) {{ {0:body} }}")]
    ProjectedFunc(DiGraph, #[kirin(type = SimpleType::F64)] ResultValue),
}

// --- Statement-level roundtrip tests ---

// --- Projected digraph roundtrip tests ---

#[test]
fn test_projected_digraph_parse_and_render() {
    let input =
        "%out = projected_func (%p0: f64) { %c = constant 1; %r = add %p0, %c; yield %r; }";
    let (stage, stmt) =
        roundtrip::emit_statement::<ProjectedDigraphLanguage>(input, &[]);
    let rendered =
        roundtrip::render_statement::<ProjectedDigraphLanguage>(&stage, stmt);

    // Verify structural elements
    assert!(rendered.contains("projected_func"), "rendered: {}", rendered);
    assert!(rendered.contains("%p0: f64"), "rendered: {}", rendered);
    assert!(rendered.contains("constant 1"), "rendered: {}", rendered);
    assert!(rendered.contains("yield %r"), "rendered: {}", rendered);
}

#[test]
fn test_projected_digraph_roundtrip_stability() {
    let input =
        "%out = projected_func (%p0: f64) { %r = add %p0, %p0; yield %r; }";
    let (stage, stmt) =
        roundtrip::emit_statement::<ProjectedDigraphLanguage>(input, &[]);
    let rendered =
        roundtrip::render_statement::<ProjectedDigraphLanguage>(&stage, stmt);

    // Second roundtrip
    let (stage2, stmt2) =
        roundtrip::emit_statement::<ProjectedDigraphLanguage>(rendered.trim(), &[]);
    let rendered2 =
        roundtrip::render_statement::<ProjectedDigraphLanguage>(&stage2, stmt2);
    assert_eq!(
        rendered.trim(),
        rendered2.trim(),
        "projected digraph roundtrip should be stable"
    );
}

#[test]
fn test_projected_digraph_empty_body_roundtrip() {
    let input = "%out = projected_func (%p0: f64) { yield %p0; }";
    let (stage, stmt) =
        roundtrip::emit_statement::<ProjectedDigraphLanguage>(input, &[]);
    let rendered =
        roundtrip::render_statement::<ProjectedDigraphLanguage>(&stage, stmt);

    let (stage2, stmt2) =
        roundtrip::emit_statement::<ProjectedDigraphLanguage>(rendered.trim(), &[]);
    let rendered2 =
        roundtrip::render_statement::<ProjectedDigraphLanguage>(&stage2, stmt2);
    assert_eq!(rendered.trim(), rendered2.trim());
}

// --- Pipeline-level roundtrip tests (auto-create staged function) ---

#[test]
fn test_specialize_without_stage_auto_creates() {
    use kirin::ir::{Pipeline, StageInfo};
    use kirin::parsers::ParsePipelineText;
    use kirin_test_languages::CallableLanguage;

    let mut pipeline: Pipeline<StageInfo<CallableLanguage>> = Pipeline::new();
    pipeline.add_stage().stage(StageInfo::default()).name("A").new();

    // No `stage` declaration — specialize auto-creates the staged function
    let input = "specialize @A fn @foo(i32) -> i32 { ^bb0(%x: i32) { ret %x; } }";
    let functions = pipeline.parse(input).expect("should parse without stage declaration");
    assert_eq!(functions.len(), 1, "should create one function");
}

#[test]
fn test_specialize_without_stage_roundtrip() {
    use kirin::ir::{Pipeline, StageInfo};
    use kirin::parsers::ParsePipelineText;
    use kirin_prettyless::PrettyPrintExt;
    use kirin_test_languages::CallableLanguage;

    // First parse: with explicit stage declaration
    let mut pipeline: Pipeline<StageInfo<CallableLanguage>> = Pipeline::new();
    pipeline.add_stage().stage(StageInfo::default()).name("A").new();

    let input = r#"
stage @A fn @foo(i32) -> i32;
specialize @A fn @foo(i32) -> i32 { ^bb0(%x: i32) { ret %x; } }
"#;
    pipeline.parse(input).expect("should parse");

    // Print and reparse for stability
    let printed = pipeline.sprint();
    let mut pipeline2: Pipeline<StageInfo<CallableLanguage>> = Pipeline::new();
    pipeline2.add_stage().stage(StageInfo::default()).name("A").new();
    pipeline2.parse(printed.trim()).expect("should reparse");
    let printed2 = pipeline2.sprint();

    assert_eq!(printed.trim(), printed2.trim(), "roundtrip should be stable");
}

// --- Full digraph roundtrip tests ---

#[test]
fn test_digraph_add_roundtrip() {
    roundtrip::assert_statement_roundtrip::<DiGraphLanguage>(
        "%r = add %a, %b",
        &[("a", SimpleType::F64), ("b", SimpleType::F64)],
    );
}

#[test]
fn test_digraph_constant_roundtrip() {
    roundtrip::assert_statement_roundtrip::<DiGraphLanguage>("%c = constant 3.14", &[]);
}

#[test]
fn test_digraph_body_parse_and_render() {
    // Parse a digraph body and verify it renders correctly
    let input =
        "%out = graph_func digraph ^dg0(%p0: f64) { %c = constant 1; %r = add %p0, %c; yield %r; }";
    let (stage, stmt) = roundtrip::emit_statement::<DiGraphLanguage>(input, &[]);
    let rendered = roundtrip::render_statement::<DiGraphLanguage>(&stage, stmt);

    // Verify key structural elements are present
    assert!(rendered.contains("digraph ^dg0"));
    assert!(rendered.contains("constant 1"));
    assert!(rendered.contains("add"));
    assert!(rendered.contains("yield %r"));

    // With Unresolved(Result) → Result resolution in the statement builder,
    // topological sort now works correctly: definitions come before uses in DAGs.
    // So reparsing the rendered output should succeed.
    let (stage2, stmt2) = roundtrip::emit_statement::<DiGraphLanguage>(rendered.trim(), &[]);
    let rendered2 = roundtrip::render_statement::<DiGraphLanguage>(&stage2, stmt2);
    assert_eq!(
        rendered.trim(),
        rendered2.trim(),
        "multi-node digraph roundtrip should be stable"
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
    assert_eq!(
        rendered.trim(),
        rendered2.trim(),
        "roundtrip should be stable"
    );
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
fn test_digraph_forward_reference_roundtrip() {
    // Forward reference: add uses %c before it's defined (reversed from input order)
    // This tests the relaxed dominance support in the parser.
    let input =
        "%out = graph_func digraph ^dg0(%p0: f64) { %r = add %p0, %c; %c = constant 1; yield %r; }";
    let (stage, stmt) = roundtrip::emit_statement::<DiGraphLanguage>(input, &[]);
    let rendered = roundtrip::render_statement::<DiGraphLanguage>(&stage, stmt);

    let (stage2, stmt2) = roundtrip::emit_statement::<DiGraphLanguage>(rendered.trim(), &[]);
    let rendered2 = roundtrip::render_statement::<DiGraphLanguage>(&stage2, stmt2);
    assert_eq!(
        rendered.trim(),
        rendered2.trim(),
        "forward-ref digraph roundtrip should be stable"
    );
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
