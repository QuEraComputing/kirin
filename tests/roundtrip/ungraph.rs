use kirin::prelude::*;
use kirin_test_languages::SimpleType;
use kirin_test_utils::roundtrip;

/// Edge operation — creates an edge SSAValue.
#[derive(Debug, Clone, PartialEq, Dialect, HasParser, PrettyPrint)]
#[kirin(edge, builders, type = SimpleType, crate = kirin::ir)]
#[chumsky(crate = kirin::parsers)]
#[chumsky(format = "{res:name} = {.wire}")]
struct Wire {
    #[kirin(type = SimpleType::Any)]
    res: ResultValue,
}

/// Unary node — one captured parameter plus one edge port.
#[derive(Debug, Clone, PartialEq, Dialect, HasParser, PrettyPrint)]
#[kirin(builders, type = SimpleType, crate = kirin::ir)]
#[chumsky(crate = kirin::parsers)]
#[chumsky(format = "{.node_a}({param}, {port})")]
struct NodeA {
    param: SSAValue,
    port: SSAValue,
}

/// Ungraph test language composing edge and node.
#[derive(Debug, Clone, PartialEq, Dialect, HasParser, PrettyPrint)]
#[kirin(builders, type = SimpleType, crate = kirin::ir)]
#[chumsky(crate = kirin::parsers)]
#[wraps]
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
