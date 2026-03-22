# Parser Scope Guards and Panic-to-Result Conversions

**Finding(s):** P1-5, P1-6, P1-7, P1-9
**Wave:** 3
**Agent role:** Implementer
**Estimated effort:** moderate

---

## Issue

### P1-5: Graph emit error paths leak relaxed-dominance mode

**File:** `crates/kirin-chumsky/src/ast/graphs.rs:192-198, 298-309`

In `DiGraph::emit_with` and `UnGraph::emit_with`, `set_relaxed_dominance(true)` is called before emitting statements. If any `emit_statement` call returns `Err`, the `?` operator propagates the error without calling `set_relaxed_dominance(false)` or `pop_scope()`. If the caller catches this error and continues using the same `EmitContext`, all subsequent SSA lookups operate in relaxed-dominance mode, silently creating forward-reference placeholders instead of reporting `UndefinedSSA`.

### P1-6: Region emit error path leaks scope

**File:** `crates/kirin-chumsky/src/ast/blocks.rs:249-281`

`Region::emit_with` calls `push_scope()` at line 249 but if `register_block` (line 261) or `emit_block` (line 270) fails with `?`, `pop_scope()` at line 281 is never called. If the `EmitContext` is reused after the error, all subsequent lookups see a phantom inner scope. Same class of bug as P1-5.

### P1-7: `parse_text.rs` panics on `link()` failure

**File:** `crates/kirin-chumsky/src/function_text/parse_text.rs:377, 432`

Two calls to `self.link(function, stage_id, staged_function)` use `.expect("link should succeed ...")`. If the link invariant is violated, this panics inside a parsing API that returns `Result`.

### P1-9: `fn_symbol` panics on unnamed functions

**File:** `crates/kirin-chumsky/src/function_text/parse_text.rs:808-813`

`fn_symbol` calls `.expect("stage declarations should always use named functions")`. While current code paths ensure the function was created with a name, this is a debug assertion in production code.

**Why grouped:** All four findings are in kirin-chumsky. P1-5 and P1-6 are the same class of bug (scope/state leak on error path), requiring RAII guards on `EmitContext`. P1-7 and P1-9 are panic-to-Result conversions in parse_text.rs. P1-5 and P1-6 require test-first approach per user decision.

**Crate(s):** kirin-chumsky
**File(s):**
- `crates/kirin-chumsky/src/ast/graphs.rs`
- `crates/kirin-chumsky/src/ast/blocks.rs`
- `crates/kirin-chumsky/src/function_text/parse_text.rs`
- `crates/kirin-chumsky/src/traits/emit_ir.rs` (EmitContext)
**Confidence:** confirmed (all)

## Guiding Principles

- "Chumsky Parser Conventions" -- `EmitContext`'s scope stack with shadowing semantics correctly models nested scoping. The relaxed-dominance mode for graph bodies is well-motivated.
- "less standalone function is better" -- the RAII guard should be a method on `EmitContext` returning a guard type.
- "No unsafe code."

## Scope

**Files to modify:**
| File | Change Type | Description |
|------|-------------|-------------|
| `crates/kirin-chumsky/src/traits/emit_ir.rs` | modify | Add `ScopeGuard` type and `scoped()` method; add `RelaxedDominanceGuard` and `relaxed_dominance_scope()` method |
| `crates/kirin-chumsky/src/ast/graphs.rs` | modify | Replace manual set/unset with RAII guard in DiGraph::emit_with and UnGraph::emit_with |
| `crates/kirin-chumsky/src/ast/blocks.rs` | modify | Replace manual push/pop with RAII guard in Region::emit_with |
| `crates/kirin-chumsky/src/function_text/parse_text.rs` | modify | Replace `.expect()` with `.map_err()` at lines ~377, ~432, ~808-813 |

**Files explicitly out of scope:**
- `crates/kirin-chumsky/src/function_text/parse_text.rs:778, 819` -- P1-8 was REMOVED (factually incorrect finding)

## Verify Before Implementing

- [ ] **Verify: `set_relaxed_dominance` is a method on EmitContext**
  Run: Grep for `set_relaxed_dominance` in `crates/kirin-chumsky/src/`
  Expected: Method on `EmitContext` that sets a boolean flag

- [ ] **Verify: `push_scope` and `pop_scope` are methods on EmitContext**
  Run: Grep for `push_scope` in `crates/kirin-chumsky/src/traits/emit_ir.rs`
  Expected: Public methods on `EmitContext`

- [ ] **Verify: `link` method returns Result**
  Run: Grep for `fn link` in `crates/kirin-chumsky/src/function_text/parse_text.rs`
  Expected: Returns `Result<..., ...>`

- [ ] **Verify: existing tests pass**
  Run: `cargo nextest run -p kirin-chumsky`
  Expected: All tests pass

## Regression Test (P0/P1 findings)

- [ ] **Write regression test for P1-5/P1-6: scope leak on emit error**
  Create a test that:
  1. Creates an `EmitContext`
  2. Pushes a scope (or enters relaxed dominance mode)
  3. Simulates a failure that would cause `?` propagation
  4. Checks that the scope was properly cleaned up (scope depth returns to original, relaxed_dominance is false)

  This tests the RAII guard behavior. The test should verify that after an error, the EmitContext state is consistent.
  Test file: `crates/kirin-chumsky/src/traits/emit_ir.rs` (inline `#[cfg(test)]`) or `crates/kirin-chumsky/src/ast/` test module.

- [ ] **Run the test -- confirm it demonstrates the issue (or test the guard directly)**
  Run: `cargo nextest run -p kirin-chumsky -E 'test(scope_guard)'`
  Expected: The test verifies guard cleanup behavior.

- [ ] **Write test for P1-9: fn_symbol on unnamed function**
  Create a test that calls `fn_symbol` (or its replacement) with an unnamed function and verifies it returns an error rather than panicking.
  Test file: `crates/kirin-chumsky/src/function_text/parse_text.rs` (inline test or existing test module)

## Implementation Steps

- [ ] **Step 1: Create ScopeGuard and RelaxedDominanceGuard types**
  In `crates/kirin-chumsky/src/traits/emit_ir.rs`, add:
  - A `ScopeGuard<'a, 'b, IR>` struct that holds `&'a mut EmitContext<'b, IR>` and calls `pop_scope()` on `Drop`.
  - A `relaxed_dominance_scope()` method on `EmitContext` that returns a guard setting relaxed dominance to `true` and restoring to `false` on drop.
  - A `scoped()` method that calls `push_scope()` and returns the `ScopeGuard`.

  Note: Since the guard holds `&mut EmitContext`, it must be the only reference during its lifetime. The `emit_with` methods will need to call methods on the guard (which derefs to `EmitContext`) instead of directly on `ctx`.

  Alternative approach if `&mut` borrow conflicts: Use `Cell<bool>` for the relaxed_dominance flag (it's already a simple boolean), and pass `&EmitContext` to the guard. Or use a simpler pattern: wrap the fallible body in a closure and run cleanup in the outer function unconditionally after the closure returns.

- [ ] **Step 2: Update DiGraph::emit_with to use guards**
  In `crates/kirin-chumsky/src/ast/graphs.rs`, replace the manual `push_scope()` / `set_relaxed_dominance(true)` / ... / `set_relaxed_dominance(false)` / `pop_scope()` with RAII guards. Ensure the guard is dropped before phase 4+ which needs `ctx` access.

- [ ] **Step 3: Update UnGraph::emit_with similarly**
  Same pattern as step 2 for the ungraph emit.

- [ ] **Step 4: Update Region::emit_with to use ScopeGuard**
  In `crates/kirin-chumsky/src/ast/blocks.rs`, replace manual `push_scope()` / `pop_scope()` with the scope guard.

- [ ] **Step 5: Convert link() panics to Results in parse_text.rs**
  At lines ~377 and ~432, replace:
  ```rust
  .expect("link should succeed for valid function")
  ```
  with:
  ```rust
  .map_err(|e| FunctionParseError::new(FunctionParseErrorKind::EmitFailed, Some(head.stage.span), format!("link failed: {e}")))?;
  ```

- [ ] **Step 6: Convert fn_symbol to return Result**
  Change `fn fn_symbol<S>(pipeline: &Pipeline<S>, function: Function) -> GlobalSymbol` to return `Result<GlobalSymbol, FunctionParseError>`. Replace the `.expect(...)` with `ok_or_else(|| FunctionParseError::new(...))`. Update all call sites to propagate with `?`.

- [ ] **Step 7: Run all tests**
  Run: `cargo nextest run -p kirin-chumsky`
  Expected: All tests pass

- [ ] **Step 8: Run clippy**
  Run: `cargo clippy -p kirin-chumsky`
  Expected: No warnings

## Must Not Do

- Do NOT introduce `#[allow(...)]` annotations to suppress warnings.
- Do NOT change the public API of `EmitContext::push_scope()` / `pop_scope()` / `set_relaxed_dominance()` -- they should remain available for direct use. The guards are additions, not replacements.
- Do NOT modify P1-8 locations (`collect_function_lookup`, `collect_staged_lookup`) -- that finding was REMOVED as factually incorrect.
- Do NOT change any parser combinator types or `HasParser` trait signatures.
- No unsafe code.

## Validation

**Per-step checks:**
- After step 1: `cargo check -p kirin-chumsky` -- Expected: compiles
- After steps 2-4: `cargo check -p kirin-chumsky` -- Expected: compiles
- After steps 5-6: `cargo check -p kirin-chumsky` -- Expected: compiles

**Final checks:**
```bash
cargo clippy -p kirin-chumsky                # Expected: no warnings
cargo nextest run -p kirin-chumsky           # Expected: all tests pass
cargo nextest run --workspace                # Expected: no regressions (downstream crates)
cargo test --doc -p kirin-chumsky            # Expected: all doctests pass
```

**Snapshot tests:** If snapshot tests exist, run `cargo insta test -p kirin-chumsky` and report changes.

## Success Criteria

1. EmitContext scope and relaxed-dominance state is always correctly restored on error paths, enforced by RAII guards.
2. `link()` failures in `parse_text.rs` return proper `FunctionParseError` instead of panicking.
3. `fn_symbol` returns `Result` instead of panicking on unnamed functions.
4. No regressions in workspace tests.

**Is this a workaround or a real fix?**
This is the real fix. RAII guards are the standard Rust pattern for ensuring cleanup on all exit paths. Converting panics to Results aligns with the crate's error philosophy.
