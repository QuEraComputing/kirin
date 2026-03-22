# Printer Fixes

**Finding(s):** P1-17, P2 (name resolution dedup, %name dedup, float NaN)
**Wave:** 3
**Agent role:** Implementer
**Estimated effort:** moderate

---

## Issue

### P1-17: `bat::print_str` panics via `.unwrap()` on I/O error

**File:** `crates/kirin-prettyless/src/bat.rs:14`

`print_str` calls `.print().unwrap()` on the bat `PrettyPrinter`. If stdout is a broken pipe (common when piping to `head` or `less` that exits early), this will panic. The callers (`FunctionRenderBuilder::bat` and `Document::pager`) already return `Result`, so propagating the error is straightforward.

### P2: Name resolution `^name` pattern duplicated 3x

**File:** `crates/kirin-prettyless/src/document/ir_render.rs:97-105, 196-204, 218-226`

The pattern for resolving block/graph names via the symbol table appears in `print_block`, `print_digraph`, and `print_ungraph`. All three follow identical logic.

### P2: `%name: Type` formatting duplicated 4x

**File:** `crates/kirin-prettyless/src/document/ir_render.rs:118, 172, 516, 530`

The SSA binding format pattern appears in block argument printing, inline port printing, `print_block_args_only`, and `print_port_list`.

### P2: Float PrettyPrint does not handle NaN or infinity

**File:** `crates/kirin-prettyless/src/impls.rs:196`

The float impl does not explicitly handle `NaN` or `Inf`. While it does not crash, the output may not roundtrip through the parser.

**Why grouped:** All findings are in kirin-prettyless. The deduplication items touch the same file (ir_render.rs). P1-17 is in bat.rs (separate file, no overlap).

**Crate(s):** kirin-prettyless
**File(s):**
- `crates/kirin-prettyless/src/bat.rs`
- `crates/kirin-prettyless/src/document/ir_render.rs`
- `crates/kirin-prettyless/src/impls.rs`
**Confidence:** confirmed (all)

## Guiding Principles

- "less standalone function is better" -- extract helper methods on the existing `IRRenderCtx` type, not standalone functions.
- "No unsafe code."
- User decision: "Prefer impl functions over standalone, regular definitions over macros, use trait/type generics when consolidating."

## Scope

**Files to modify:**
| File | Change Type | Description |
|------|-------------|-------------|
| `crates/kirin-prettyless/src/bat.rs` | modify | Return `Result<(), io::Error>` from `print_str` |
| `crates/kirin-prettyless/src/document/ir_render.rs` | modify | Extract `resolve_caret_name` and `print_typed_ssa_binding` helpers; refactor callers |
| `crates/kirin-prettyless/src/impls.rs` | modify | Add explicit NaN/Inf branches in float PrettyPrint |
| `crates/kirin-prettyless/src/pipeline.rs` | modify | Update callers of `print_str` if needed |

**Files explicitly out of scope:**
- `crates/kirin-prettyless/src/pipeline.rs:123, 164` -- #[must_use] additions are in LHF
- `crates/kirin-prettyless/Cargo.toml` -- bat default-features is in LHF (LHF-2)

## Verify Before Implementing

- [ ] **Verify: `print_str` currently returns `()`**
  Run: Read `crates/kirin-prettyless/src/bat.rs`
  Expected: `pub(crate) fn print_str(s: &str)` with no return type

- [ ] **Verify: callers of `print_str` return Result**
  Run: Grep for `print_str` in `crates/kirin-prettyless/src/`
  Expected: Callers are in Result-returning functions

- [ ] **Verify: existing tests pass**
  Run: `cargo nextest run -p kirin-prettyless`
  Expected: All tests pass

## Regression Test (P0/P1 findings)

- [ ] **P1-17 regression test is not feasible in unit test form**
  The bug manifests as a panic on broken pipe / I/O error, which is hard to simulate in a test. The fix is mechanical (`.unwrap()` -> `?`). Validation will be by code review and compilation.

## Implementation Steps

- [ ] **Step 1: Fix bat::print_str to return Result**
  Change `print_str` signature to `pub(crate) fn print_str(s: &str) -> Result<(), std::io::Error>`. Replace `.unwrap()` with `?`. The bat `print()` returns `Result<bool>` -- map the error with `.map_err(...)` or use `?` directly if `bat::Error` converts to `io::Error`. Drop the `bool` return value (it indicates whether paging was used).

- [ ] **Step 2: Update callers of print_str**
  Update `Document::pager` in `bat.rs` to propagate the new `Result`. Update `FunctionRenderBuilder::bat` in `pipeline.rs` if it calls `print_str`.

- [ ] **Step 3: Extract `resolve_caret_name` helper method**
  On `IRRenderCtx` (or the appropriate type in `ir_render.rs`), add:
  ```rust
  fn resolve_caret_name(&self, name: Option<Symbol>, fallback: impl std::fmt::Display) -> String {
      name.and_then(|name_sym| self.stage.symbol_table().resolve(name_sym).map(|s| format!("^{}", s)))
          .unwrap_or_else(|| format!("{}", fallback))
  }
  ```
  Refactor `print_block`, `print_digraph`, and `print_ungraph` to use this helper.

- [ ] **Step 4: Extract `format_typed_ssa_binding` helper**
  Add a helper that formats `%name: Type` for an SSA value. Refactor the 4 duplicate sites to use it.

- [ ] **Step 5: Add explicit NaN/Inf handling in float PrettyPrint**
  In `crates/kirin-prettyless/src/impls.rs`, add branches at the top of the float `PrettyPrint` impl:
  ```rust
  if self.is_nan() { return doc.text("nan"); }
  if self.is_infinite() { return doc.text(if self.is_sign_positive() { "inf" } else { "-inf" }); }
  ```
  Add test cases for NaN and infinity.

- [ ] **Step 6: Run all tests**
  Run: `cargo nextest run -p kirin-prettyless`
  Expected: All tests pass

- [ ] **Step 7: Run clippy**
  Run: `cargo clippy -p kirin-prettyless`
  Expected: No warnings

## Must Not Do

- Do NOT introduce `#[allow(...)]` annotations.
- Do NOT change the public API signature of `Document::pager` (it already returns `Result`).
- Do NOT modify LHF items (bat default-features, #[must_use] additions, print_ports dedup).
- Do NOT create standalone functions -- use methods on existing types.
- No unsafe code.

## Validation

**Per-step checks:**
- After step 1-2: `cargo check -p kirin-prettyless --features bat` -- Expected: compiles
- After step 3-4: `cargo check -p kirin-prettyless` -- Expected: compiles
- After step 5: `cargo nextest run -p kirin-prettyless` -- Expected: all tests pass including new NaN/Inf tests

**Final checks:**
```bash
cargo clippy -p kirin-prettyless             # Expected: no warnings
cargo nextest run -p kirin-prettyless        # Expected: all tests pass
cargo nextest run --workspace                # Expected: no regressions
cargo test --doc -p kirin-prettyless         # Expected: all doctests pass
```

**Snapshot tests:** Yes -- if snapshot tests exist for printer output, run `cargo insta test -p kirin-prettyless` and report changes. Do NOT auto-accept.

## Success Criteria

1. `bat::print_str` returns `Result` and never panics on I/O errors.
2. Name resolution patterns (`^name` and `%name: Type`) are each defined once and reused.
3. Float printing explicitly handles NaN and infinity with documented formatting.
4. No regressions.

**Is this a workaround or a real fix?**
This is the real fix. Converting unwrap to Result is the correct error handling. Deduplication follows the project's preference for impl methods over standalone functions. NaN/Inf handling is additive correctness.
