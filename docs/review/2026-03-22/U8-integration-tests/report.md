# U8: Integration Tests — Code Quality Review

**Scope:** `tests/simple.rs`, `tests/compile_fail.rs`, `tests/compile-fail/`, `tests/roundtrip/` (all files)
**Perspective:** Code Quality
**Total lines reviewed:** ~1,955
**Date:** 2026-03-22

---

## Findings

### [P2] [high] Dead code: `strip_trailing_whitespace` is defined but never called — composite.rs:8

**Perspective:** Code Quality

The function `strip_trailing_whitespace` is defined with `#[allow(dead_code)]` but is never called anywhere in the test suite. The `#[allow(dead_code)]` suppresses the warning rather than addressing the root issue.

**Suggested action:** Remove the function entirely. If it was intended for future use, it should be added when actually needed.

---

### [P2] [high] Unused `Result` values from `register_ssa` — composite.rs:53-54, composite.rs:143

**Perspective:** Code Quality

Three calls to `emit_ctx.register_ssa(...)` ignore the returned `Result`. These could silently fail (e.g., duplicate SSA name registration) without the test noticing. The compiler emits `unused_must_use` warnings for these.

```
emit_ctx.register_ssa("a".to_string(), ssa_a);
emit_ctx.register_ssa("b".to_string(), ssa_b);
...
emit_ctx.register_ssa("v".to_string(), ssa_v);
```

**Suggested action:** Unwrap or expect the results: `emit_ctx.register_ssa(...).expect("register should succeed");`

---

### [P2] [high] Five unused `PrettyPrintExt` imports in digraph.rs — digraph.rs:127, 218, 282, 346, 434

**Perspective:** Code Quality

Five test functions import `kirin_prettyless::PrettyPrintExt` but never use it. The `sprint()` calls in these tests operate on `Pipeline` via `PipelinePrintExt` (which is automatically available), not `PrettyPrintExt`. The compiler emits warnings for each.

**Suggested action:** Remove all five `use kirin_prettyless::PrettyPrintExt;` lines.

---

### [P2] [medium] `composite.rs` manually reimplements roundtrip logic that exists in `kirin-test-utils` — composite.rs:27-261

**Perspective:** Code Quality

The `composite.rs` file (262 lines) manually implements the parse-emit-print roundtrip pattern using low-level APIs (`parse_ast`, `EmitContext`, `Document`), including a local `test_ssa_kind` helper. Meanwhile, `kirin-test-utils::roundtrip` provides `emit_statement`, `render_statement`, and `assert_statement_roundtrip` that encapsulate this exact pattern. Tests in `arith.rs`, `bitwise.rs`, `cmp.rs`, `function.rs`, and `namespace.rs` all use the shared utilities.

The manual approach in `composite.rs` tests `SimpleLanguage` which uses a custom `$` prefix format and a function variant with Region, making it slightly different from the standard dialect pattern. However, `test_roundtrip_add`, `test_roundtrip_constant`, and `test_roundtrip_return` could all use `roundtrip::assert_statement_roundtrip::<SimpleLanguage>`.

**Suggested action:** Migrate the simple statement-level tests (`test_roundtrip_add`, `test_roundtrip_constant`, `test_roundtrip_return`) to use `roundtrip::assert_statement_roundtrip`. Keep the function-level tests (`test_roundtrip_function`, `test_roundtrip_function_multiple_blocks`) manual if needed due to the SSA type-patching workaround (lines 186-190, 239-243).

---

### [P2] [medium] Misplaced tests: `test_specialize_without_stage_*` in `digraph.rs` use `CallableLanguage`, not digraph types — digraph.rs:103-160

**Perspective:** Code Quality

Two tests (`test_specialize_without_stage_auto_creates` and `test_specialize_without_stage_roundtrip`) use `CallableLanguage` and test pipeline auto-creation behavior. They have nothing to do with directed graphs or any digraph-specific functionality. Their placement in `digraph.rs` is misleading.

**Suggested action:** Move these tests to a new file (e.g., `tests/roundtrip/pipeline.rs`) or into `tests/roundtrip/function.rs` since they test pipeline-level parsing behavior with `CallableLanguage`.

---

### [P2] [medium] `cf.rs` tests only assert parse succeeds, never verify output — cf.rs:5-91

**Perspective:** Code Quality

All four `cf.rs` tests follow the same pattern: parse input, assert the result is non-empty, and stop. They never verify the parsed IR content or roundtrip the output. This is noted in `MEMORY.md` ("CF branch tests: `Successor::Display` outputs raw block IDs (`^0`) while block headers use symbolic names -- causes roundtrip mismatch"), but the tests could still verify structural properties of the parsed output (e.g., number of blocks, number of statements, terminator presence).

```rust
let parsed = pipeline.parse(input).expect("parse should succeed");
assert!(!parsed.is_empty(), "should parse at least one function");
```

Compare to every other dialect roundtrip test which either does exact string comparison or at minimum a two-pass stability check.

**Suggested action:** Add structural assertions (e.g., verify block count, check that terminators exist, verify the parsed function's signature). If full roundtrip is not feasible due to the known block ID mismatch, document the limitation with a comment and add a tracking issue.

---

### [P3] [high] `NumericLanguage` enum defined identically in both `arith.rs` and `bitwise.rs` — arith.rs:9-19, bitwise.rs:10-20

**Perspective:** Code Quality

Both files define a `NumericLanguage` enum with nearly identical structure (the only difference is swapping `Arith` for `Bitwise` in one variant). The `test_composes_with_constant_and_control_flow` functions in both files are structurally identical: create constants, create an operation, create a return, finalize, then assert matches/properties on each statement. This is ~70 lines of duplicated test logic.

Since these are test-local enums used only for composition testing (not reusable test languages), they cannot be extracted to `kirin-test-languages` without creating coupling. The duplication is minor and contained.

**Suggested action:** Consider extracting a parameterized composition test helper into `kirin-test-utils` that takes a dialect-specific operation builder closure. Alternatively, accept the duplication as the cost of test isolation.

---

### [P3] [high] Weak assertions in `test_roundtrip_function` — composite.rs:203-213

**Perspective:** Code Quality

`test_roundtrip_function` uses `contains`-based assertions instead of exact string comparison:

```rust
assert!(buf.contains("add"), "Should have add instruction");
assert!(buf.contains("constant 42"), "Should have constant instruction");
assert!(buf.contains("return"), "Should have return instruction");
```

The companion test `test_roundtrip_function_multiple_blocks` (line 217) uses exact comparison (`assert_eq!(output.trim_end(), input)`). The weaker test at line 165 accepts any output containing the keywords "add", "constant 42", "return" in any arrangement, which could pass even with malformed output.

**Suggested action:** Align `test_roundtrip_function` with `test_roundtrip_function_multiple_blocks` by using `assert_eq!` for exact comparison. If the output format is unstable (e.g., indentation), use insta snapshots or normalize whitespace before comparing.

---

### [P3] [high] Debug `println!` statements left in test code — simple.rs:63, composite.rs:258

**Perspective:** Code Quality

Two test functions contain `println!` for debug output. These produce noise when running the test suite and are typically indicative of debug code that was not cleaned up.

**Suggested action:** Remove both `println!` calls, or replace with comments explaining what was being verified.

---

### [P3] [medium] No roundtrip test for `scf::For` — scf.rs

**Perspective:** Code Quality

The `kirin-scf` crate defines three operations: `If`, `For`, and `Yield`. The integration roundtrip tests in `scf.rs` cover `If` and `Yield` (the `test_if_roundtrip` and `test_yield_in_if_roundtrip` tests) but have no test for the `For` loop construct. The `For` operation has a more complex format (`for %iv in %lo..%hi step %s do { ... }`) with four SSA operands plus a block body, making it a higher-risk surface for parser/printer bugs.

**Suggested action:** Add a `test_for_roundtrip` test covering the `For` operation with its range and step operands.

---

### [P3] [medium] `cmp.rs` does not verify dialect properties unlike parallel tests — cmp.rs:5-59

**Perspective:** Code Quality

The `arith.rs` and `bitwise.rs` roundtrip tests verify `is_pure()` and `is_speculatable()` properties of each operation as part of their `assert_roundtrip` helper. The `cmp.rs` roundtrip tests only verify text equality and do not check these properties. While `kirin-cmp` has internal unit tests for these properties, the inconsistency in integration test coverage is a gap — internal tests verify properties on the raw `Cmp` type, but integration tests should verify properties survive the parse-emit-finalize pipeline.

**Suggested action:** Add property assertions to the `assert_cmp_roundtrip` helper (both `is_pure` and `is_speculatable` should be true for all cmp operations).

---

### [P3] [medium] Repetitive pipeline setup boilerplate in `digraph.rs` — digraph.rs (13 occurrences)

**Perspective:** Code Quality

The pattern `Pipeline::new()` + `add_stage().stage(StageInfo::default()).name("test").new()` is repeated 13 times in `digraph.rs`. Many of these tests define custom test-local dialect enums that cannot use `roundtrip::assert_pipeline_roundtrip` (which requires `ParsePipelineText` blanket impl). However, a local helper function like `fn make_pipeline<L>() -> Pipeline<StageInfo<L>>` with the stage name as a parameter would eliminate the boilerplate.

**Suggested action:** Extract a local `make_pipeline(name: &str) -> Pipeline<StageInfo<L>>` helper in `digraph.rs` to reduce the repeated 5-line setup blocks.

---

### [P4] [medium] `simple.rs::test_block` has weak `contains`-based assertions — simple.rs:65-68

**Perspective:** Code Quality

The `test_block` function in `simple.rs` uses four `assert!(buf.contains(...))` checks that only verify keyword presence, not structural correctness. This test exercises the builder API (creating blocks, regions, functions) and pretty printing, but the assertions would pass even if the output format were severely broken (e.g., instructions in the wrong order, missing block headers).

**Suggested action:** Replace `contains` assertions with an exact string comparison or an insta snapshot. The test already renders to a string; comparing against expected output provides much stronger guarantees.

---

### [P4] [low] Inconsistent test naming conventions across roundtrip files

**Perspective:** Code Quality

Test naming patterns vary across files:
- `arith.rs`: `test_roundtrip_all_operations_with_integer_types` (descriptive, grouped)
- `cmp.rs`: `test_eq_roundtrip`, `test_ne_roundtrip` (per-operation, short)
- `function.rs`: `test_bind_roundtrip_with_multiple_captures` (per-operation, descriptive)
- `cf.rs`: `test_branch_parse`, `test_diamond_control_flow` (inconsistent, no "roundtrip" suffix)
- `composite.rs`: `test_roundtrip_add`, `test_roundtrip_function` (generic prefix)

There is no established naming convention, which makes it harder to understand test coverage at a glance from test names alone.

**Suggested action:** Adopt a consistent pattern like `test_<dialect>_<operation>_<level>` (e.g., `test_arith_add_statement_roundtrip`, `test_cf_branch_pipeline_parse`). This is low priority and could be addressed incrementally.

---

## Summary

| Priority | Count | Key themes |
|----------|-------|------------|
| P2       | 5     | Compiler warnings (dead code, unused imports, unused Result), manual reimplementation of shared utilities, misplaced tests |
| P3       | 5     | Missing For test, weak assertions, debug println, property coverage gap, boilerplate |
| P4       | 2     | Weak assertions in simple.rs, naming inconsistency |

The integration test suite provides solid roundtrip coverage for most dialects. The primary issues are (1) compiler warnings that should be cleaned up (dead code, unused imports, unhandled Results), (2) the `cf.rs` tests being parse-only with no output verification, (3) missing coverage for `scf::For`, and (4) `composite.rs` using manual low-level APIs instead of the shared roundtrip utilities.
