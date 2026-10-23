use crate::stage::Stage;
use kirin::prelude::*;

/// Parse a one-function ZX program with `body` as its UnGraph body, returning
/// the error message.
fn parse_error(body: &str) -> String {
    let text = format!(
        "stage @zx fn @f() -> Qubit;\nspecialize @zx fn @f() -> Qubit ungraph ^ug0 {{\n{body}\n}}\n"
    );
    let mut ir: Pipeline<Stage> = Pipeline::new();
    let error = ir.parse(&text).expect_err("the body is invalid");
    format!("{error:?}")
}

#[test]
fn an_edge_statement_without_the_edge_keyword_is_a_parse_error() {
    let error = parse_error("  %w0 = wire -> Qubit;\n  z_spider (0.0) %w0;");
    assert!(
        error.contains("an edge statement without `edge`"),
        "{error}"
    );
}

#[test]
fn the_edge_keyword_on_a_node_is_a_parse_error() {
    let error = parse_error("  edge %w0 = wire -> Qubit;\n  edge z_spider (0.0) %w0;");
    assert!(error.contains("`edge` on a non-edge statement"), "{error}");
}
