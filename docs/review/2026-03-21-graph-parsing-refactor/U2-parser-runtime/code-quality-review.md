# U2: Parser Runtime (kirin-chumsky + kirin-lexer) -- Code Quality Review

## Clippy / Lint Findings

### [P2] [confirmed] #[allow(dead_code)] at tests.rs:385 (TestDialect enum)
Root cause: `TestDialect` is used only for `BuilderStageInfo<TestDialect>` construction in tests; the enum variants themselves are never pattern-matched. Removable: yes. Fix: Add `#[cfg(test)]` on the entire block (already present) and change to `#[expect(dead_code, reason = "test-only minimal dialect")]` for documentation. Alternatively, the single `Noop` variant could be replaced with a unit struct via a custom Dialect impl.

### [P2] [confirmed] #[allow(dead_code)] at function_text/syntax.rs:14,16 (Header.stage, Header.function)
Root cause: `Header.stage` and `Header.function` are parsed from the token stream but consumed only by position (the caller extracts `signature` and `span`). The stage/function names are used indirectly via span-based re-parsing in pass 2. Removable: uncertain -- these fields document the grammar structure. Fix: If truly unused, prefix with `_` instead of `#[allow]`. If retained for documentation, switch to `#[expect(dead_code, reason = "parsed for grammar completeness")]`.

## Duplication Findings

### [P2] [confirmed] port_list() vs capture_list() -- parsers/graphs.rs:16-27 vs :37-48
Lines duplicated: 12. These two functions are token-for-token identical except for the `.labelled()` string. Suggested abstraction: A single `named_arg_list(label: &str)` function. Lines saved: ~10.

### [P2] [likely] Statement-semicolon parsing pattern repeated 5+ times
The pattern `language.clone().map_with(|stmt, e| Spanned { value: stmt, span: e.span() }).then_ignore(just(Token::Semicolon)).repeated().collect::<Vec<_>>()` appears in `block()`, `digraph()`, `digraph_body_statements()`, `block_body_statements()` and similar. Suggested abstraction: A `statement_list(language)` helper. Lines saved: ~20.

## Rust Best Practices

### [P1] [likely] parse_text.rs at 978 lines -- decomposition opportunity
This file contains both `ParsePipelineText` and `ParseStatementText` traits, pass-1 logic, pass-2 logic, error formatting, stage dispatch, and function lookup. The two-pass architecture is well-documented but the single file makes navigation difficult. Suggested split: `parse_pipeline.rs` (pipeline trait + passes), `parse_statement.rs` (statement trait), `lookup.rs` (function/stage lookup helpers).

### [P3] [uncertain] No #[must_use] on parser combinator return types
Most `pub fn` parser constructors (e.g., `block()`, `region()`, `digraph()`) return parser values that are useless if discarded. Chumsky parsers are lazy, so dropping them silently loses work. Low priority since Chumsky's own types may already handle this.

## Strengths

- Two-pass pipeline parsing architecture is well-documented with clear module-level doc comments explaining the design rationale.
- Graph parser component functions (`port_list`, `capture_list`, `yield_type_list`, `digraph_body_statements`, `ungraph_body_statements`) provide good composability for format-string projections.
- `body_span` scanner cleanly handles brace-balanced region skipping without parsing contents.
- Test organization in `tests.rs` is methodical with macro helpers for consistent test patterns.
