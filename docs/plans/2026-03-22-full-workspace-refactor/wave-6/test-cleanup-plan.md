# Test Cleanup

**Finding(s):** Theme H (U8 integration test findings), P2 UnitTy dedup, P2 misplaced tests, P3 test improvements
**Wave:** 6
**Agent role:** Implementer
**Estimated effort:** moderate

---

## Issue

This plan addresses the integration test cleanup findings from U8 and the cross-dialect UnitTy deduplication from U6.

### P2: Duplicated `UnitTy` test helper across 6 dialect crates

**Files:** `kirin-cf/src/tests.rs`, `kirin-scf/src/tests.rs`, `kirin-bitwise/src/tests.rs`, `kirin-cmp/src/tests.rs`, `kirin-function/src/call.rs`, `kirin-function/src/ret.rs`

Every dialect's tests define an identical `UnitTy` struct. Meanwhile, `kirin-test-types` already exports `UnitType` with the same purpose.

### P2: `composite.rs` manually reimplements roundtrip logic

**File:** `tests/roundtrip/composite.rs:27-261`

Manual parse-emit-print using low-level APIs instead of the shared `kirin-test-utils::roundtrip` utilities. Simple statement-level tests could use `assert_statement_roundtrip`.

### P2: Misplaced tests in `digraph.rs`

**File:** `tests/roundtrip/digraph.rs:103-160`

`test_specialize_without_stage_*` tests use `CallableLanguage` and test pipeline behavior, not digraph functionality.

### P2: `cf.rs` tests only assert parse succeeds

**File:** `tests/roundtrip/cf.rs:5-91`

All four tests follow: parse input, assert non-empty, stop. No output verification.

### P3: Weak assertions in composite.rs and simple.rs

Contains-based assertions instead of exact comparison. Debug println left in test code.

### P3: Missing roundtrip test for `scf::For`

No For operation roundtrip test.

### P3: `cmp.rs` does not verify dialect properties

Unlike arith.rs and bitwise.rs which check `is_pure()` and `is_speculatable()`.

### P3: Repetitive pipeline setup in digraph.rs

13 occurrences of the same pipeline creation pattern.

**Crate(s):** tests/, kirin-cf, kirin-scf, kirin-bitwise, kirin-cmp, kirin-function
**File(s):** Multiple test files across the workspace
**Confidence:** high (all)

## Guiding Principles

- "Test Conventions" -- Roundtrip tests go in `tests/roundtrip/`. Unit tests go inline. New test types go in `kirin-test-types`. New test helpers go in `kirin-test-utils`.
- "when creating tests, always put common tools created for testing in the `kirin-test-utils` crate, unless they are specific to a single crate."

## Scope

**Files to modify:**
| File | Change Type | Description |
|------|-------------|-------------|
| `crates/kirin-cf/src/tests.rs` | modify | Replace local UnitTy with kirin-test-types::UnitType |
| `crates/kirin-scf/src/tests.rs` | modify | Replace local UnitTy with UnitType |
| `crates/kirin-bitwise/src/tests.rs` | modify | Replace local UnitTy with UnitType |
| `crates/kirin-cmp/src/tests.rs` | modify | Replace local UnitTy with UnitType |
| `crates/kirin-function/src/call.rs` | modify | Replace local UnitTy with UnitType |
| `crates/kirin-function/src/ret.rs` | modify | Replace local UnitTy with UnitType |
| `crates/kirin-cf/Cargo.toml` | modify | Add kirin-test-types dev-dependency |
| `crates/kirin-scf/Cargo.toml` | modify | Add kirin-test-types dev-dependency |
| `crates/kirin-bitwise/Cargo.toml` | modify | Add kirin-test-types dev-dependency |
| `crates/kirin-cmp/Cargo.toml` | modify | Add kirin-test-types dev-dependency |
| `crates/kirin-function/Cargo.toml` | modify | Add kirin-test-types dev-dependency |
| `tests/roundtrip/composite.rs` | modify | Migrate simple tests to use roundtrip utilities; strengthen assertions |
| `tests/roundtrip/digraph.rs` | modify | Move pipeline tests to appropriate file; extract pipeline helper |
| `tests/roundtrip/cf.rs` | modify | Add structural assertions |
| `tests/roundtrip/scf.rs` | modify | Add For roundtrip test |
| `tests/roundtrip/cmp.rs` | modify | Add property assertions |

**Files explicitly out of scope:**
- `tests/simple.rs:63` -- println removal is in LHF (LHF-13)
- `tests/roundtrip/composite.rs:258` -- println removal is in LHF (LHF-13)
- `tests/roundtrip/composite.rs:8` -- dead code removal is in LHF (LHF-14)
- `tests/roundtrip/composite.rs:53-54, 143` -- register_ssa Result handling is in LHF (LHF-15)
- `tests/roundtrip/digraph.rs` unused imports -- in LHF (LHF-16)

## Verify Before Implementing

- [ ] **Verify: kirin-test-types exports UnitType with required traits**
  Run: Grep for `pub struct UnitType` in `crates/kirin-test-types/src/`
  Expected: UnitType exists with Debug, Clone, Hash, PartialEq, Eq, Default, Display

- [ ] **Verify: kirin-test-types is compatible as dev-dep for dialect crates**
  Run: Check that kirin-test-types does not depend on any dialect crate (no cycle)
  Expected: No circular dependency

- [ ] **Verify: existing tests pass**
  Run: `cargo nextest run --workspace`
  Expected: All tests pass

## Implementation Steps

### Part A: UnitTy Deduplication

- [ ] **Step 1: Add kirin-test-types as dev-dependency to dialect crates**
  Add `kirin-test-types = { path = "../kirin-test-types" }` under `[dev-dependencies]` in each dialect crate's Cargo.toml.

- [ ] **Step 2: Replace local UnitTy with UnitType in each dialect test file**
  In each file, replace the local `UnitTy` struct and its impls with `use kirin_test_types::UnitType;`. Update all references from `UnitTy` to `UnitType`.

  **Note:** `UnitType` displays as "()" while the local `UnitTy` displays as "unit". Existing tests use `{:?}` (Debug) format and check for variant names (e.g., `contains("Branch")`), not type Display output, so this difference should not cause failures. If any test does depend on the Display output, update the assertion to match "()".

- [ ] **Step 3: Verify dialect tests pass**
  Run: `cargo nextest run -p kirin-cf -p kirin-scf -p kirin-bitwise -p kirin-cmp -p kirin-function`
  Expected: All tests pass

### Part B: Integration Test Improvements

- [ ] **Step 4: Add structural assertions to cf.rs**
  For each cf.rs test, add assertions beyond "parsed non-empty": verify block count, terminator presence, or statement count. This addresses the parse-only gap without requiring full roundtrip.

- [ ] **Step 5: Verify For roundtrip test exists in scf.rs (added by Wave 5a)**
  Wave 5a (scf-result-values-plan) adds a For roundtrip test with init_args and results. Verify it exists and covers the basic `For` operation. If additional coverage is needed (e.g., edge cases like step=1, empty init_args), add supplementary tests. Do NOT duplicate the Wave 5a roundtrip test.

- [ ] **Step 6: Add property assertions to cmp.rs**
  Add `is_pure()` and `is_speculatable()` checks to the `assert_cmp_roundtrip` helper, matching the pattern in arith.rs and bitwise.rs.

- [ ] **Step 7: Move misplaced pipeline tests from digraph.rs**
  Move `test_specialize_without_stage_auto_creates` and `test_specialize_without_stage_roundtrip` to `tests/roundtrip/function.rs` (or a new `tests/roundtrip/pipeline.rs`).

- [ ] **Step 8: Extract pipeline helper in digraph.rs**
  Create a local helper `fn make_test_pipeline<L>() -> Pipeline<StageInfo<L>>` and refactor the repeated 5-line setup blocks.

- [ ] **Step 9: Strengthen composite.rs assertions**
  For `test_roundtrip_function`, replace `contains`-based assertions with exact string comparison or `assert_eq!`, matching the pattern in `test_roundtrip_function_multiple_blocks`.

- [ ] **Step 10: Run all tests**
  Run: `cargo nextest run --workspace`
  Expected: All tests pass

- [ ] **Step 11: Run clippy**
  Run: `cargo clippy --workspace`
  Expected: No warnings in test code

## Must Not Do

- Do NOT introduce `#[allow(...)]` annotations.
- Do NOT modify LHF items (println removal, dead code removal, unused imports, register_ssa).
- Do NOT change the public API of any production crate -- this is test-only.
- Do NOT remove tests -- only improve or relocate them.
- Do NOT add kirin-test-types as a regular (non-dev) dependency to dialect crates.
- No unsafe code.

## Validation

**Per-step checks:**
- After step 2: `cargo nextest run -p kirin-cf -p kirin-scf -p kirin-bitwise -p kirin-cmp -p kirin-function` -- Expected: PASS
- After step 5: `cargo nextest run --test scf` -- Expected: PASS
- After step 7: `cargo nextest run --test digraph --test function` -- Expected: PASS

**Final checks:**
```bash
cargo clippy --workspace                     # Expected: no warnings
cargo nextest run --workspace                # Expected: all tests pass
cargo test --doc --workspace                 # Expected: all doctests pass
```

**Snapshot tests:** No snapshot changes expected. If any, run `cargo insta test --workspace` and report.

## Success Criteria

1. All 6 local `UnitTy` definitions are replaced with `UnitType` from `kirin-test-types`.
2. cf.rs tests have structural assertions beyond "parsed non-empty".
3. scf.rs has a For roundtrip test (added by Wave 5a; verify it exists, add supplementary edge case tests if needed).
4. cmp.rs tests verify dialect properties.
5. Misplaced pipeline tests are in the right file.
6. composite.rs uses exact assertions where possible.
7. No test regressions.

**Is this a workaround or a real fix?**
This is the real fix. Deduplicating UnitTy and improving test assertions are permanent improvements to code quality and test coverage.
