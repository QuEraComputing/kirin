# Continuation Enum + run_nested_calls + eval_block + Abstract Interpreter Changes

**Finding(s):** W3, W4, W5, W6
**Wave:** 1
**Agent role:** Implementer
**Estimated effort:** moderate

---

## Issue

Four tightly coupled changes in the interpreter framework must happen atomically:

**W3 — Continuation enum changes**: The `Yield`, `Return`, and `Call` variants must change from single-value to multi-value:
- `Yield(V)` -> `Yield(SmallVec<[V; 1]>)`
- `Return(V)` -> `Return(SmallVec<[V; 1]>)`
- `Call { result: ResultValue }` -> `Call { results: SmallVec<[ResultValue; 1]> }`

**W4 — run_nested_calls changes**: Return type changes from `Result<V, E>` to `Result<SmallVec<[V; 1]>, E>`. The `pending_results` stack changes from `Vec<ResultValue>` to `Vec<SmallVec<[ResultValue; 1]>>`. Multi-result writeback with arity check.

**W5 — eval_block changes**: `eval_block` wraps `run_nested_calls` result in `Yield(values)` — now a SmallVec.

**W6 — Abstract interpreter changes**: `AnalysisResult.return_value: Option<V>` becomes `return_values: Option<SmallVec<[V; 1]>>`. `propagate_control` performs pointwise join with arity check. `is_subseteq` uses pointwise comparison. The abstract `eval_block` Call handler writes back all return values from the analysis result to the Call's result slots.

These are coupled because changing the Continuation enum immediately breaks all match sites in both the concrete and abstract interpreters, and the AnalysisResult must match the new Continuation shape.

**Crate(s):** kirin-interpreter
**File(s):**
- `crates/kirin-interpreter/src/control.rs` — Continuation enum (lines 19-51)
- `crates/kirin-interpreter/src/stack/exec.rs` — run_nested_calls (lines 95-137)
- `crates/kirin-interpreter/src/stack/frame.rs` — eval_block (lines 132-158)
- `crates/kirin-interpreter/src/stack/transition.rs` — advance_frame_with_stage match (lines 90-115)
- `crates/kirin-interpreter/src/stack/stage.rs` — Staged::advance match (lines 26-37), Staged::call return type (line 40)
- `crates/kirin-interpreter/src/stack/call.rs` — call_with_stage return type (line 45), CallSemantics blanket impls (lines 45-77, 83-161)
- `crates/kirin-interpreter/src/call.rs` — CallSemantics trait Result type (line 17), SSACFGRegion blanket impls
- `crates/kirin-interpreter/src/abstract_interp/interp.rs` — abstract eval_block Call handling (lines 329-345)
- `crates/kirin-interpreter/src/abstract_interp/fixpoint.rs` — propagate_control match (lines 252-278), run_forward return_value (line 149)
- `crates/kirin-interpreter/src/block_eval.rs` — BlockEvaluator trait docs (line 54)
- `crates/kirin-interpreter/tests/stack_interp.rs` — test match sites (lines 127, 190, 291)
- `crates/kirin-interpreter/tests/stage_dispatch.rs` — test Call construction (line 121)

**Confidence:** confirmed

## Guiding Principles

- "Interpreter Conventions": The interpreter framework uses three composable sub-traits: `ValueStore`, `StageAccess<'ir>`, `BlockEvaluator<'ir>`. Dialect authors use `I: Interpreter<'ir>`.
- "`'ir` lifetime pattern": `StageAccess<'ir>` and `BlockEvaluator<'ir>` are parameterized by `'ir`.
- "No unsafe code": All implementations MUST use safe Rust.
- Arity guardrails: All Return/Yield sites must agree on arity. Mismatches are hard errors (`InterpreterError::ArityMismatch`).

## Scope

**Files to modify:**
| File | Change Type | Description |
|------|-------------|-------------|
| `crates/kirin-interpreter/src/control.rs` | modify | Change Yield, Return, Call variants to SmallVec |
| `crates/kirin-interpreter/src/stack/exec.rs` | modify | Change run_nested_calls return type and pending_results |
| `crates/kirin-interpreter/src/stack/frame.rs` | modify | Update eval_block to wrap SmallVec result |
| `crates/kirin-interpreter/src/stack/transition.rs` | modify | Update match arms for new variant shapes |
| `crates/kirin-interpreter/src/stack/stage.rs` | modify | Update Staged::advance match, Staged::call return type |
| `crates/kirin-interpreter/src/stack/call.rs` | modify | Update call_with_stage return type, StackInterpreter::call return type |
| `crates/kirin-interpreter/src/stack/dispatch.rs` | modify | Update CallDynAction Result/Output types to SmallVec |
| `crates/kirin-interpreter/src/call.rs` | modify | Update CallSemantics StackInterpreter blanket impl Result type |
| `crates/kirin-interpreter/src/abstract_interp/interp.rs` | modify | Update abstract eval_block Call handling for multi-result |
| `crates/kirin-interpreter/src/abstract_interp/fixpoint.rs` | modify | Update propagate_control for SmallVec, update run_forward return type |
| `crates/kirin-interpreter/src/result.rs` | modify | Change return_value to return_values: Option<SmallVec<[V; 1]>> |
| `crates/kirin-interpreter/src/block_eval.rs` | modify | Update doc comments |
| `crates/kirin-interpreter/tests/stack_interp.rs` | modify | Update match arms and `.call()` return handling |
| `crates/kirin-interpreter/tests/stage_dispatch.rs` | modify | Update Call construction |
| `crates/kirin-interpreter/tests/derive_macros.rs` | modify | Update `.call()` return handling (lines 169, 184) |
| `crates/kirin-interpreter/src/abstract_interp/summary.rs` | modify | Update tests that use return_value() |

**Files explicitly out of scope:**
- `crates/kirin-scf/` — dialect interpret impls updated in wave-2
- `crates/kirin-function/` — dialect interpret impls updated in wave-2
- `crates/kirin-derive-interpreter/` — derive codegen may need updates but only if it generates Continuation constructors (verify)

## Verify Before Implementing

- [ ] **Verify: Continuation enum is at the expected location**
  Run: `grep -n "pub enum Continuation" crates/kirin-interpreter/src/control.rs`
  Expected: Single hit around line 19.

- [ ] **Verify: run_nested_calls signature**
  Run: `grep -n "fn run_nested_calls" crates/kirin-interpreter/src/stack/exec.rs`
  Expected: Single hit around line 95, returning `Result<V, E>`.

- [ ] **Verify: AnalysisResult::return_value field name**
  Run: `grep -n "return_value" crates/kirin-interpreter/src/result.rs`
  Expected: Multiple hits — field declaration, constructor, accessor.

- [ ] **Verify: derive-interpreter does NOT generate Continuation constructors**
  Run: `grep -rn "Continuation" crates/kirin-derive-interpreter/`
  Expected: If the derive generates Continuation::Yield or Return, those codegen sites need updating too. If not, derive-interpreter is out of scope.

- [ ] **Verify: All Continuation match sites in kirin-interpreter**
  Run: `grep -rn "Continuation::" crates/kirin-interpreter/src/ | grep -v "^.*:.*//"`
  Expected: All match sites are in the files listed above. If additional files match, add them to scope.

## Regression Test

- [ ] **Write test for multi-result writeback in run_nested_calls**
  Add a test in `tests/stack_interp.rs` (or a new test file) that:
  1. Constructs a function returning multiple values via `Return(smallvec![v1, v2])`
  2. Calls it with `Call { results: smallvec![rv1, rv2] }`
  3. Verifies both result values are written correctly
  Before W3 changes: this won't compile (Continuation variants don't accept SmallVec).

- [ ] **Write test for arity mismatch detection**
  Add a test that constructs a `Return(smallvec![v1, v2])` paired with `Call { results: smallvec![rv1] }` (arity 2 vs 1) and verifies `ArityMismatch` error.

## Implementation Steps

- [ ] **Step 1: Add `write_results` helper function**
  Add a standalone `pub` helper in `control.rs` (alongside the `Continuation` enum) and re-export from `lib.rs`:
  ```rust
  // In control.rs:
  pub fn write_results<S>(
      store: &mut S,
      results: &[ResultValue],
      values: &SmallVec<[S::Value; 1]>,
  ) -> Result<(), S::Error>
  where
      S: ValueStore,
      S::Error: From<InterpreterError>,
  {
      if results.len() != values.len() {
          return Err(InterpreterError::ArityMismatch {
              expected: results.len(), got: values.len(),
          }.into());
      }
      for (rv, val) in results.iter().zip(values.iter()) {
          store.write(*rv, val.clone())?;
      }
      Ok(())
  }
  ```
  In `lib.rs`:
  - Add to the `pub use control::` line (line 26): `pub use control::{Args, ConcreteExt, Continuation, write_results};`
  - Add `write_results` to the `prelude` module (line 44-47) so dialect authors get it via `use kirin_interpreter::prelude::*;`
  This is needed so that downstream dialect crates (`kirin-scf`, `kirin-function`, `kirin-unpack`) can import and use `write_results` for arity-checked result writeback.

- [ ] **Step 2: Change Continuation enum variants**
  In `control.rs`:
  - `Return(V)` -> `Return(SmallVec<[V; 1]>)`
  - `Yield(V)` -> `Yield(SmallVec<[V; 1]>)`
  - `Call { result: ResultValue }` -> `Call { results: SmallVec<[ResultValue; 1]> }`
  Update doc comments.

- [ ] **Step 3: Fix all compiler errors in kirin-interpreter**
  After step 2, `cargo check -p kirin-interpreter` will show all broken match sites. Fix them one by one:

  **exec.rs (run_nested_calls):**
  - Return type: `Result<V, E>` -> `Result<SmallVec<[V; 1]>, E>`
  - `pending_results`: `Vec<ResultValue>` -> `Vec<SmallVec<[ResultValue; 1]>>`
  - `Call { result, .. }` -> `Call { results, .. }` + `pending_results.push(results.clone())`
  - `Return(v)` / `Yield(v)` -> `Return(ref values)` / `Yield(ref values)` + arity-checked writeback

  **frame.rs (eval_block):**
  - `Ok(Continuation::Yield(v))` already works (v is now SmallVec)

  **transition.rs (advance_frame_with_stage):**
  - `Continuation::Return(_)` — pattern unchanged (just destructures)
  - `Continuation::Yield(_)` — pattern unchanged
  - `Continuation::Call { .. }` — already uses `..`

  **stage.rs (Staged::advance and Staged::call):**
  - `Continuation::Call { callee, stage, args, .. }` — already uses `..`, no change needed for advance
  - `Staged::call` (line 40): return type changes from `Result<V, E>` to `Result<SmallVec<[V; 1]>, E>` because it delegates to `call_with_stage` which delegates to `CallSemantics::eval_call`

  **call.rs (CallSemantics trait and SSACFGRegion blanket impls):**
  - `CallSemantics` trait (line 17): `type Result` associated type itself does NOT change (it remains abstract). But the StackInterpreter blanket impl at line 53 changes `type Result = V` to `type Result = SmallVec<[V; 1]>` because `run_nested_calls` now returns `SmallVec<[V; 1]>`.
  - The AbstractInterpreter blanket impl at line 91 is unchanged: `type Result = AnalysisResult<V>` (AnalysisResult internally stores `return_values: Option<SmallVec<[V; 1]>>`).

  **stack/call.rs:**
  - `call_with_stage` (line 45): return type changes from `Result<V, E>` to `Result<SmallVec<[V; 1]>, E>`, matching the new `CallSemantics::Result` for StackInterpreter.
  - `StackInterpreter::call` (line 27): return type also changes to `Result<SmallVec<[V; 1]>, E>`.

  **stack/dispatch.rs:**
  - `CallDynAction` (line 62): `CallSemantics<..., Result = V>` bound changes to `Result = SmallVec<[V; 1]>`.
  - `type Output = V` (line 65) changes to `type Output = SmallVec<[V; 1]>`.
  - The return type of `CallDynAction::run` (line 72) follows from the Output type change.

  **abstract_interp/interp.rs (abstract eval_block, lines 331-345):**
  - `Continuation::Call { result, .. }` -> `Call { results, .. }`
  - Currently (line 343): `let return_val = analysis.return_value().cloned().unwrap_or_else(V::bottom); self.write(result, return_val)?;`
  - After: Extract `return_values: Option<SmallVec<[V; 1]>>` from the analysis result. If `Some(values)`, zip with `results` and write each one (arity check: `values.len() == results.len()`). If `None`, write `V::bottom()` to each result slot.
  - Use `write_results` helper if the abstract interpreter has `ValueStore` in scope; otherwise inline the zip+write loop.

- [ ] **Step 4: Update AnalysisResult for multi-result**
  In `result.rs`:
  - `return_value: Option<V>` -> `return_values: Option<SmallVec<[V; 1]>>`
  - Update constructor: `return_value: Option<V>` param -> `return_values: Option<SmallVec<[V; 1]>>`
  - `bottom()`: unchanged (return_values remains `None`)
  - Rename `return_value()` accessor to `return_values()` returning `Option<&SmallVec<[V; 1]>>`
  - Add backward-compat `return_value()` that returns `self.return_values.as_ref().and_then(|vs| vs.first())`
  - `is_subseteq` (lines 87-96): Change from single-value comparison to pointwise comparison:
    ```rust
    match (&self.return_values, &other.return_values) {
        (Some(a), Some(b)) => {
            if a.len() != b.len() { return false; }
            for (av, bv) in a.iter().zip(b.iter()) {
                if !av.is_subseteq(bv) { return false; }
            }
        }
        (Some(_), None) => return false,
        _ => {}
    }
    ```
  - `Clone`, `Debug`: mechanical — just rename field

- [ ] **Step 5: Update propagate_control in fixpoint.rs (lines 252-278)**
  - `Continuation::Return(v) | Continuation::Yield(v)` -> `Return(values) | Yield(values)` where `values: SmallVec<[V; 1]>`
  - Currently (line 246): `return_value: &mut Option<V>` parameter. Change to `return_values: &mut Option<SmallVec<[V; 1]>>`
  - Currently (line 149): `let mut return_value: Option<V> = None;`. Change to `let mut return_values: Option<SmallVec<[V; 1]>> = None;`
  - Pointwise join logic (matching design doc "Abstract Interpreter" section):
    ```rust
    match (&mut *return_values, values) {
        (None, vs) => *return_values = Some(vs.clone()),
        (Some(existing), vs) if existing.len() != vs.len() => {
            return Err(InterpreterError::ArityMismatch {
                expected: existing.len(), got: vs.len(),
            }.into());
        }
        (Some(existing), vs) => {
            for (e, v) in existing.iter_mut().zip(vs.iter()) {
                *e = if narrowing { e.narrow(v) } else { e.join(v) };
            }
        }
    }
    ```

- [ ] **Step 6: Update summary.rs tests**
  Tests that construct `AnalysisResult::new(..., Some(Interval::constant(42)))` need updating to use `Some(smallvec![Interval::constant(42)])`.

- [ ] **Step 7: Update stack_interp.rs and derive_macros.rs tests**
  - `Continuation::Return(v)` match arms -> `Continuation::Return(values)` + extract first value via `values.into_iter().next().unwrap()` or index `values[0]`
  - All `.call(callee, &args)` invocations now return `SmallVec<[V; 1]>` — extract single value where needed (e.g., `interp.in_stage::<L>().call(sf, &[10i64]).unwrap()[0]` or `...unwrap().into_iter().next().unwrap()`)
  - `tests/derive_macros.rs` lines 169 and 184 also call `.call()` and need the same treatment
  - Add multi-result test case

- [ ] **Step 8: Update stage_dispatch.rs tests**
  - `Continuation::Call { result: self.result() }` -> `Call { results: smallvec![self.result()] }`

- [ ] **Step 9: Update downstream dialect interpret impls temporarily**
  `kirin-scf` and `kirin-function` construct `Continuation::Yield(v)`, `Return(v)`, and `Call { result }`. These need mechanical updates to compile:

  **kirin-scf (`crates/kirin-scf/src/interpret_impl.rs`):**
  - Line 206: `Continuation::Yield(value)` match -> `Continuation::Yield(values)` + extract `values.into_iter().next().unwrap()`
  - Line 207: `interp.write(self.result, value)?` -> `interp.write(self.result, values.into_iter().next().unwrap())?` (single-result compat)
  - Line 248: same pattern for For's yield handler
  - Line 264: `interp.write(self.result, value)?` -> same pattern
  - Line 285: `Continuation::Yield(v)` -> `Continuation::Yield(smallvec![v])`

  **kirin-function (`crates/kirin-function/src/interpret_impl.rs`):**
  - Line 253: `result: self.result()` -> `results: smallvec![self.result()]`
  - Line 271: `Continuation::Return(v)` -> `Continuation::Return(smallvec![v])`

  **kirin-interpreter tests (`crates/kirin-interpreter/tests/stage_dispatch.rs`):**
  - Line 121-125: `result: self.result()` -> `results: smallvec![self.result()]`

  **example/toy-lang/src/main.rs:**
  - Line 107: `interp.in_stage::<HighLevel>().call(spec, &args)?` now returns `SmallVec<[V; 1]>` — extract single value
  - Line 118: same for `LowLevel`

  These are minimal compatibility fixes. Full multi-result dialect changes happen in wave-2.

- [ ] **Step 10: Run all tests**
  Run: `cargo nextest run -p kirin-interpreter`
  Expected: All tests pass.

- [ ] **Step 11: Run workspace build**
  Run: `cargo build --workspace`
  Expected: Clean build. Dialect crates compile with mechanical updates from step 9.

- [ ] **Step 12: Run workspace tests**
  Run: `cargo nextest run --workspace`
  Expected: All tests pass.

- [ ] **Step 13: Fix clippy warnings**
  Run: `cargo clippy -p kirin-interpreter`
  Expected: No warnings.

## Must Not Do

- Do NOT introduce NEW `#[allow(...)]` annotations to suppress warnings — fix the underlying cause. Note: `frame.rs` (line 141) and `block_eval.rs` (line 60) already have `#[allow(clippy::multiple_bound_locations)]` for `L: Dialect` on method + where clause — these are pre-existing and should be preserved, not removed.
- Do NOT leave clippy warnings.
- Do NOT implement full multi-result semantics in SCF/function dialects here — just mechanical compatibility. Full changes are in wave-2.
- Do NOT change the builder template or format DSL — those are in wave-0 plans.
- No unsafe code (AGENTS.md: all implementations MUST use safe Rust).
- Do NOT use `mem::transmute` or similar to convert between old and new Continuation shapes.

## Validation

**Per-step checks:**
- After step 3: `cargo check -p kirin-interpreter` — Expected: compiles (may have warnings)
- After step 9: `cargo check --workspace` — Expected: compiles
- After step 10: `cargo nextest run -p kirin-interpreter` — Expected: all pass
- After step 12: `cargo nextest run --workspace` — Expected: all pass

**Final checks:**
```bash
cargo clippy -p kirin-interpreter             # Expected: no warnings
cargo clippy --workspace                       # Expected: no new warnings
cargo nextest run --workspace                  # Expected: all tests pass
cargo test --doc --workspace                   # Expected: all doctests pass
```

**Snapshot tests:** yes — run `cargo insta test -p kirin-interpreter` if snapshots exist.

## Success Criteria

1. `Continuation::Yield`, `Return`, and `Call` carry `SmallVec<[V; 1]>` / `SmallVec<[ResultValue; 1]>`.
2. `run_nested_calls` performs arity-checked multi-result writeback on Return.
3. `AnalysisResult` stores `return_values: Option<SmallVec<[V; 1]>>` with pointwise join in `propagate_control`.
4. All existing tests pass (with updated match arms).
5. New tests verify multi-result writeback and arity mismatch detection.
6. `write_results` helper is available for dialect interpret impls.
7. Workspace builds and all tests pass.

**Is this a workaround or a real fix?**
This is the real fix. The Continuation enum change is the foundational framework change that enables all downstream multi-result support.
