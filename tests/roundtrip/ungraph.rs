use kirin::prelude::*;
use kirin_test_languages::SimpleType;
use kirin_test_utils::roundtrip;

/// Edge operation — creates an edge SSAValue.
#[derive(Debug, Clone, PartialEq, Dialect, HasParser, PrettyPrint)]
#[kirin(edge, builders, type = SimpleType, crate = kirin::ir)]
#[chumsky(crate = kirin::parsers)]
#[chumsky(format = "$wire")]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
struct Wire {
    #[kirin(type = SimpleType::Any)]
    res: ResultValue,
}

/// Unary node — one captured parameter plus one edge port.
#[derive(Debug, Clone, PartialEq, Dialect, HasParser, PrettyPrint)]
#[kirin(builders, type = SimpleType, crate = kirin::ir)]
#[chumsky(crate = kirin::parsers)]
#[chumsky(format = "$node_a({param}, {port})")]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
struct NodeA {
    param: SSAValue,
    port: SSAValue,
}

/// Ungraph test language composing edge and node.
#[derive(Debug, Clone, PartialEq, Dialect, HasParser, PrettyPrint)]
#[kirin(builders, type = SimpleType, crate = kirin::ir)]
#[chumsky(crate = kirin::parsers)]
#[wraps]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
enum UngraphTestLanguage {
    Wire(Wire),
    NodeA(NodeA),
}

// --- Roundtrip tests ---

#[test]
fn test_ungraph_wire_roundtrip() {
    roundtrip::assert_statement_roundtrip::<UngraphTestLanguage>("%w = wire", &[]);
}

#[test]
fn test_ungraph_node_roundtrip() {
    roundtrip::assert_statement_roundtrip::<UngraphTestLanguage>(
        "node_a(%p, %e0)",
        &[("p", SimpleType::F64), ("e0", SimpleType::Any)],
    );
}

#[test]
fn test_ungraph_body_with_a_terminator_is_a_parse_error() {
    // An UnGraph has no terminator; parsing one must report an error, not
    // panic in the builder.
    let input = r#"
stage @test fn @g(i64, i64) -> i64;
specialize @test fn @g(i64, i64) -> i64 ungraph ^u0(%x: i64, %y: i64) {
  %s = add %x, %y -> i64;
  ret %s;
}
"#;
    let mut pipeline: Pipeline<StageInfo<kirin_test_languages::GraphFunctionLanguage>> =
        Pipeline::new();
    let error = pipeline
        .parse(input)
        .expect_err("a terminator is not an ungraph member");
    assert!(
        format!("{error:?}").contains("cannot contain a terminator"),
        "unexpected error: {error:?}"
    );
}
