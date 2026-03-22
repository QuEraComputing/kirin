# Void If/For + Yield Enforcement

**Finding(s):** #3 (SCF If always requires result type) + #4 (eval_block only exits on Yield)
**Wave:** 1
**Agent role:** Implementer
**Estimated effort:** design-work

---

## Issue

**These two findings are coupled** because void-If changes yield semantics (blocks may yield nothing), which directly affects the eval_block contract.

### Finding #3: SCF `If` Always Requires a Result Type

Adding `result: ResultValue` to `If` and `For` means the parser always expects `-> <type>` in the text format. MLIR's `scf.if` and `scf.for` support zero results (void if/for). Kirin's current `If<T>` and `For<T>` always produce one result.

Full text from implementation-notes.md:
> Adding `result: ResultValue` to `If` means the parser now always expects `-> <type>` in the text format. The existing toy-lang programs used `if` in a "void" context where branches terminated with `ret` instead of `yield`.
>
> **Design note:** MLIR's `scf.if` supports zero results (void if). Kirin's current `If` always produces one result. Supporting void `if` would require making `result` optional or using a sentinel type.

### Finding #4: `eval_block` Only Exits on `Yield`

`StackInterpreter::eval_block` calls `run_nested_calls(|_interp, is_yield| is_yield)`, meaning it only returns when it receives `Continuation::Yield`. A `Return` inside an SCF body causes `NoFrame` error. SCF block bodies must terminate with `yield`, but nothing enforces this at the IR level — it's a runtime invariant.

Full text from implementation-notes.md:
> `StackInterpreter::eval_block` calls `run_nested_calls(|_interp, is_yield| is_yield)`, meaning it only returns when it receives `Continuation::Yield`. A `Return` inside an SCF body causes it to try `pop()` on an empty pending_results stack, triggering `InterpreterError::NoFrame`.
>
> **Impact:** SCF block bodies must terminate with `yield`, not `ret`. This is correct per MLIR semantics but is not enforced at the IR level — it's a runtime invariant.

### Why They're Coupled

Void If changes yield semantics: if an `scf.if` produces no result, its body blocks may `yield` with no value (or may not yield at all — but per MLIR semantics, they always yield, just with zero operands). The `eval_block` contract expects `Continuation::Yield(V)` carrying a single value. Making `result` optional means `Yield` may carry a "unit/void" value, which interacts directly with how `eval_block` and the interpret impls handle the yield continuation.

**Crate(s):** kirin-scf, kirin-interpreter, kirin-ir (for Placeholder understanding)
**File(s):**
- `crates/kirin-scf/src/lib.rs:54-97` — `If<T>`, `For<T>`, `Yield<T>` struct definitions
- `crates/kirin-scf/src/interpret_impl.rs:179-307` — interpret impls for If, For, Yield, StructuredControlFlow
- `crates/kirin-interpreter/src/block_eval.rs:50-69` — `BlockEvaluator::eval_block` trait definition
- `crates/kirin-interpreter/src/stack/frame.rs:142-157` — `StackInterpreter::eval_block`
- `crates/kirin-interpreter/src/stack/exec.rs:95-137` — `run_nested_calls`
- `crates/kirin-interpreter/src/abstract_interp/interp.rs:317-356` — `AbstractInterpreter::eval_block`
**Confidence:** confirmed

## Guiding Principles

- "Block vs Region: A `Block` is a single linear sequence of statements with an optional terminator. A `Region` is a container for multiple blocks. When modeling MLIR-style operations, check whether the MLIR op uses `SingleBlock` regions — if so, use `Block` in Kirin, not `Region`." — SCF ops correctly use `Block`.
- "Auto-placeholder for `ResultValue` fields: `ResultValue` fields without an explicit `#[kirin(type = ...)]` annotation automatically default to `ir_type::placeholder()`. Dialect authors never write `+ Placeholder` on their struct definitions." — Relevant when `result` becomes `Option<ResultValue>`.
- "Interpreter Conventions: `Continuation<V, Ext = Infallible>`: Continue, Jump, Fork, Call, Return, Yield, Ext(Ext)" — Yield carries a single `V` value. Void yield must be represented somehow.
- "No unsafe code. All implementations MUST use safe Rust."
- "Test Conventions: Roundtrip tests go in workspace `tests/roundtrip/<dialect>.rs`. Unit tests for internal logic go inline in the crate (`#[cfg(test)]`)."

## Scope

**Files to modify:**
| File | Change Type | Description |
|------|-------------|-------------|
| `crates/kirin-scf/src/lib.rs` | modify | Make `result: Option<ResultValue>` in `If<T>` and `For<T>`, update format strings |
| `crates/kirin-scf/src/interpret_impl.rs` | modify | Handle `Option<ResultValue>` in If/For interpret impls |
| `crates/kirin-interpreter/src/block_eval.rs` | doc only | Enhance `eval_block` doc comment to document yield-only contract for SCF body blocks |
| `tests/roundtrip/scf.rs` | modify | Add void-if and void-for roundtrip tests |

**Files explicitly out of scope:**
- `crates/kirin-interpreter/src/block_eval.rs` — `eval_block` trait definition and signature stay as-is. Only the doc comment is enhanced (Step 5). No code changes.
- `crates/kirin-interpreter/src/stack/frame.rs` — `StackInterpreter::eval_block` unchanged.
- `crates/kirin-interpreter/src/stack/exec.rs` — `run_nested_calls` unchanged.
- `crates/kirin-interpreter/src/abstract_interp/interp.rs` — `AbstractInterpreter::eval_block` unchanged.
- `crates/kirin-prettyless/src/impls.rs` — touched by CF roundtrip plan, not this one.

## Verify Before Implementing

- [ ] **Verify: `Option<ResultValue>` is supported by derive macros**
  Add the `Option<ResultValue>` field temporarily to `If<T>` and run `cargo check -p kirin-scf`.
  Expected: Compiles. `Option<T>` has `PrettyPrint` impl (lines 129-143 of `kirin-prettyless/src/impls.rs`), but verify that the derive macros (HasParser, PrettyPrint, Dialect) handle `Option<ResultValue>` correctly in format strings.
  **If this fails with a derive error, use the fallback approach (see Design Decisions).**

- [ ] **Verify: format string `-> {result:type}` can be made optional**
  The derive-chumsky format string for If is currently:
  `$if {condition} then {then_body} else {else_body} -> {result:type}`
  When `result` is `Option<ResultValue>`, the `-> {result:type}` portion should be optional in parsing/printing. Verify that the derive handles `Option<ResultValue>` in format strings — the field being `None` should skip printing.
  Run: `cargo check -p kirin-scf` after making the change.
  **If the derive doesn't handle optional format segments, STOP and report — this requires derive macro changes.**

- [ ] **Verify: `Yield<T>` still works for void case**
  In void-if, the body blocks should still terminate with `yield` but with no value. This means either:
  (a) `Yield<T>` gets an optional `value` field too, or
  (b) A separate `VoidYield` variant is added, or
  (c) `Yield` always carries a value and void-if uses a unit/placeholder value.
  The design decision below addresses this.

## Design Decisions

**Decision 1: How to represent optional results in `If<T>` and `For<T>`**
- **Primary approach:** Change `result: ResultValue` to `result: Option<ResultValue>`. Update the format string to make the `-> {result:type}` portion conditional. When `result` is `None`, no `-> type` is printed/parsed.
- **Fallback:** Keep `result: ResultValue` mandatory but use a sentinel void type (e.g., `()` or a `VoidType`) as the result type for void-if. This avoids any derive macro changes but is less clean.
- **How to decide:** Add `result: Option<ResultValue>` to `If<T>`, update the format string, and `cargo check -p kirin-scf`. If the derive macros reject `Option<ResultValue>` in a format string position, use the fallback.

**Decision 2: How void-yield interacts with `Continuation::Yield(V)`**
- **Primary approach:** For void-if/for, the body blocks still use `yield` with a value. The yielded value is simply discarded (not written to any ResultValue since result is `None`). The `Yield<T>` struct is unchanged — it always carries a value. In the interpret impl, when `result` is `None`, the yielded value is ignored instead of being written.
- **Fallback:** Make `Yield<T>.value` an `Option<SSAValue>` and add a void yield variant that carries no value. This requires changes to the yield interpret impl and format string.
- **How to decide:** The primary approach is simpler and avoids changing `Yield` or `Continuation`. The only question is whether MLIR allows `scf.yield` with zero operands — it does (`scf.yield` with no args for void `scf.if`). In that case, the fallback (optional yield value) is more faithful to MLIR. Choose based on whether the derive supports `Option<SSAValue>` in yield's format string cleanly.
- **Recommendation:** Start with the primary approach (always yield a value, discard on void-if). If void-yield text format is needed (e.g., `yield;` with no value), use the fallback.

**Decision 3: Whether to add yield enforcement (finding #4)**
- **Primary approach:** Document the contract more explicitly in `eval_block`'s doc comment. Add a debug assertion in the SCF interpret impls that verifies the block terminated with `Yield`. Do NOT add a full Verifier trait — that's a larger architectural addition.
- **Fallback:** If the user wants stronger enforcement, add a `validate_block_terminates_with_yield` helper in kirin-scf that checks at IR level before interpretation.
- **How to decide:** The scope of this plan is to make void-if work and document the yield contract, not to build verification infrastructure. Use the primary approach unless the user requests otherwise.

## Implementation Steps

- [ ] **Step 1: Make `result` optional in `If<T>`**
  In `crates/kirin-scf/src/lib.rs`, change:
  ```rust
  result: ResultValue,
  ```
  to:
  ```rust
  result: Option<ResultValue>,
  ```
  Update the format string from:
  ```
  $if {condition} then {then_body} else {else_body} -> {result:type}
  ```
  to something that makes `-> {result:type}` optional. This depends on how the derive handles `Option<ResultValue>` in format strings. Try:
  ```
  $if {condition} then {then_body} else {else_body} {result}
  ```
  where the `Option<ResultValue>` PrettyPrint impl handles the `-> type` rendering. **Or** if the derive doesn't support optional segments, split into two format variants.

  Run: `cargo check -p kirin-scf`
  Expected: Compiles.

- [ ] **Step 2: Make `result` optional in `For<T>`**
  Same change as Step 1 but for `For<T>`. Change `result: ResultValue` to `result: Option<ResultValue>` and update the format string.

  Run: `cargo check -p kirin-scf`
  Expected: Compiles.

- [ ] **Step 3: Update `If::interpret` for optional result**
  In `crates/kirin-scf/src/interpret_impl.rs`, the `If` interpret impl (lines 179-213) currently does:
  ```rust
  Continuation::Yield(value) => {
      interp.write(self.result, value)?;
      Ok(Continuation::Continue)
  }
  ```
  Change to:
  ```rust
  Continuation::Yield(value) => {
      if let Some(result) = self.result {
          interp.write(result, value)?;
      }
      Ok(Continuation::Continue)
  }
  ```

- [ ] **Step 4: Update `For::interpret` for optional result**
  In `crates/kirin-scf/src/interpret_impl.rs`, the `For` interpret impl (lines 215-270) has one place that writes `self.result`:
  - Lines 264-266: `if let Some(value) = carried.into_iter().next() { interp.write(self.result, value)?; }`

  Change this block (lines 263-266) to:
  ```rust
  if let Some(result) = self.result {
      if let Some(value) = carried.into_iter().next() {
          interp.write(result, value)?;
      }
  }
  ```

- [ ] **Step 5: Update `eval_block` doc comment for yield contract**
  In `crates/kirin-interpreter/src/block_eval.rs`, enhance the doc comment on `eval_block` (lines 50-57) to explicitly state:
  - SCF body blocks MUST terminate with `Yield`
  - `StackInterpreter::eval_block` will only return on `Yield` — a `Return` inside an SCF body triggers `NoFrame`
  - The `AbstractInterpreter` returns `Err(missing_terminator())` if no terminator exists

  This is documentation only, no code change.

- [ ] **Step 6: Add roundtrip test for void-if**
  In `tests/roundtrip/scf.rs`, add a test that parses a void-if (no `-> type`), prints it, and verifies the output. The test program should have an `if` with branches that yield but the `if` itself produces no result.

  Run: `cargo nextest run --test roundtrip`
  Expected: PASS.

- [ ] **Step 7: Add roundtrip test for void-for**
  Same as Step 6 but for `for` with no result.

  Run: `cargo nextest run --test roundtrip`
  Expected: PASS.

- [ ] **Step 8: Verify existing SCF tests still pass**
  Run: `cargo nextest run -p kirin-scf`
  Expected: All existing tests pass — the result-carrying If/For should still work.

- [ ] **Step 9: Run clippy and fix warnings**
  Run: `cargo clippy -p kirin-scf -p kirin-interpreter`
  Expected: No warnings.

## Must Not Do

- Do NOT introduce `#[allow(...)]` annotations to suppress warnings — fix the underlying cause.
- Do NOT leave clippy warnings. Run `cargo clippy -p kirin-scf` before reporting completion.
- Do NOT change `Continuation::Yield(V)` to `Continuation::Yield(Option<V>)` — this would cascade throughout the entire interpreter framework. The void case is handled at the SCF level by ignoring the yielded value.
- Do NOT add a full `Verifier` trait — that's out of scope for this plan. Only document the yield contract.
- Do NOT change `StackInterpreter::eval_block` or `AbstractInterpreter::eval_block` — the eval_block contract is correct. SCF ops handle the yield value.
- Do NOT change `run_nested_calls` — it correctly handles yield/return dispatch.
- Do NOT modify `Yield<T>` unless Decision 2 fallback is needed. Start with the primary approach (always yield a value, discard on void-if).
- `cargo check` failure 3x → stop and report.

## Validation

**Per-step checks:**
- After step 1: `cargo check -p kirin-scf` — Expected: compiles (or reveals derive issue → fallback)
- After step 2: `cargo check -p kirin-scf` — Expected: compiles
- After step 3+4: `cargo nextest run -p kirin-scf` — Expected: existing tests pass
- After step 6+7: `cargo nextest run --test roundtrip` — Expected: new void tests pass
- After step 8: `cargo nextest run -p kirin-scf` — Expected: all pass

**Final checks:**
```bash
cargo clippy -p kirin-scf                    # Expected: no warnings
cargo clippy -p kirin-interpreter            # Expected: no warnings
cargo nextest run -p kirin-scf               # Expected: all tests pass
cargo nextest run --workspace                # Expected: no regressions
cargo test --doc --workspace                 # Expected: all doctests pass
```

**Snapshot tests:** No snapshot tests expected for SCF changes.

## Success Criteria

1. `If<T>` and `For<T>` support optional results — `result: Option<ResultValue>` compiles and works with derive macros.
2. Void-if and void-for can be parsed and printed without `-> <type>` in the text format.
3. Existing result-carrying If/For operations still work — no regressions in SCF tests.
4. The `eval_block` doc comment clearly documents the yield-only contract for SCF body blocks.
5. Void-if interpret impl correctly ignores the yielded value when `result` is `None`.
6. The `Continuation::Yield(V)` type is unchanged — void handling is contained in kirin-scf.

**Is this a workaround or a real fix?**
This is the real fix for finding #3 (void-if support). For finding #4 (yield enforcement), this is a partial fix: we document the contract but don't add compile-time or IR-level enforcement. A full Verifier trait would be the complete fix for #4 but is deferred as out-of-scope architectural work. The documentation + existing runtime errors (`NoFrame` on `Return` inside SCF body) provide adequate protection for now.
