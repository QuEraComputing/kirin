# Backlog

## Test Simplification Opportunities (`kirin-test-utils`)

- [ ] Simplify manual test SSA creation in `tests/simple.rs:305`, `tests/simple.rs:311`, and `tests/simple.rs:404` by using `kirin_test_utils::new_test_ssa`.
- [ ] Replace local parser helper in `crates/kirin-chumsky/src/builtins.rs:318` with `kirin_test_utils::parser::parse_has_parser::<T>(input)` for shared parser test flow.
- [ ] Stabilize token snapshot tests by formatting generated tokens:
  `crates/kirin-chumsky-format/src/generics.rs:13` and `crates/kirin-derive-dialect/src/marker.rs:35`
  should use `kirin_test_utils::rustfmt`/`rustfmt_display` instead of raw `tokens.to_string()`.
- [ ] Evaluate consolidating duplicated test types in `tests/simple.rs:4` and `tests/simple.rs:98` with `kirin-test-utils`.
  Note: this is a larger refactor because current `kirin-test-utils` does not yet provide the exact parser/pretty behavior used in this file.
