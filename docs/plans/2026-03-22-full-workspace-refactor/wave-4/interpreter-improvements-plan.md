# Interpreter Improvements

**Finding(s):** P1-12, P2 (Vec alloc, frame clone, crate-level allow, FrameStack docs, propagate_control docs)
**Wave:** 4
**Agent role:** Implementer
**Estimated effort:** moderate

---

## Issue

### P1-12: `active_stage_info` panics on misconfigured pipelines

**File:** `crates/kirin-interpreter/src/stage_access.rs:36`

`active_stage_info` calls `.expect("active stage does not contain StageInfo for this dialect")`. This is the primary stage resolution path. `in_stage()` at line 71 uses this panicking version. Since `in_stage()` is heavily used by both `StackInterpreter` and `AbstractInterpreter`, a misconfigured pipeline causes an unrecoverable panic deep in the interpreter.

The fallible alternative `resolve_stage_info` exists at line 47, but `in_stage()` does not use it.

**User decision:** Provide BOTH panicking and non-panicking API -- add `try_in_stage()` that returns `Result`.

### P2: Vec<SSAValue> allocation on every `bind_block_args` call

**File:** `crates/kirin-interpreter/src/block_eval.rs:44-48`

The default implementation collects block argument SSA values into a temporary `Vec<SSAValue>` on every block entry. This allocation occurs on every block entry for both concrete and abstract paths. Could use direct `zip` iteration instead.

### P2: Same Vec<SSAValue> allocation in `propagate_block_args`

**File:** `crates/kirin-interpreter/src/abstract_interp/fixpoint.rs:319-326`

Same pattern as above, in a hot path during fixpoint iteration.

### P2: `run_forward` clones frame values at completion

**File:** `crates/kirin-interpreter/src/abstract_interp/fixpoint.rs:188-191`

After the worklist drains, the analysis result clones the frame's `values` HashMap and `block_args` HashMap. The frame is about to be popped, so consuming it via `into_parts()` would avoid the clone.

### P2: Crate-level `#![allow(...)]` suppresses 3 clippy lints globally

**File:** `crates/kirin-interpreter/src/lib.rs:3-7`

Three lints are suppressed globally. They should be targeted to specific modules/items.

### P2: FrameStack::read documentation gap

**File:** `crates/kirin-interpreter/src/frame_stack.rs:85-89`

`read` only looks at the top frame. This is correct for SSA semantics but should be documented.

### P2: `propagate_control` Call no-op documentation gap

**File:** `crates/kirin-interpreter/src/abstract_interp/fixpoint.rs:274`

`Continuation::Call` is a no-op in `propagate_control` because it's handled inline in `eval_block`. Needs a comment.

**Why grouped:** All findings are in kirin-interpreter. P1-12 is the primary fix; the P2 items are cleanup/performance improvements in the same crate. No file overlaps with other wave plans.

**Crate(s):** kirin-interpreter
**File(s):**
- `crates/kirin-interpreter/src/stage_access.rs`
- `crates/kirin-interpreter/src/block_eval.rs`
- `crates/kirin-interpreter/src/abstract_interp/fixpoint.rs`
- `crates/kirin-interpreter/src/lib.rs`
- `crates/kirin-interpreter/src/frame_stack.rs`
**Confidence:** confirmed (all)

## Guiding Principles

- "Interpreter Conventions" -- `StageAccess<'ir>` is parameterized by `'ir`. `in_stage()` is the primary API for dialect authors. The trait decomposition is `ValueStore` / `StageAccess` / `BlockEvaluator` / `Interpreter`.
- P1-12 user decision: "Provide BOTH panicking and non-panicking API variants (e.g., `in_stage` + `try_in_stage`)."
- "No unsafe code."

## Scope

**Files to modify:**
| File | Change Type | Description |
|------|-------------|-------------|
| `crates/kirin-interpreter/src/stage_access.rs` | modify | Add `try_in_stage()` method returning `Result<Staged, E>` |
| `crates/kirin-interpreter/src/block_eval.rs` | modify | Replace Vec collect with direct zip iteration |
| `crates/kirin-interpreter/src/abstract_interp/fixpoint.rs` | modify | Replace Vec collect with zip; consume frame instead of cloning; add Call comment |
| `crates/kirin-interpreter/src/lib.rs` | modify | Move crate-level allows to specific items |
| `crates/kirin-interpreter/src/frame_stack.rs` | modify | Add doc comment on `read` |

**Files explicitly out of scope:**
- `crates/kirin-interpreter/src/control.rs` -- #[must_use] is in LHF (LHF-1)
- `crates/kirin-interpreter/src/result.rs` -- assert upgrade is in LHF (LHF-17)

## Verify Before Implementing

- [ ] **Verify: `in_stage` currently panics**
  Run: Read `crates/kirin-interpreter/src/stage_access.rs` lines 70-81
  Expected: `in_stage` calls `active_stage_info` which panics

- [ ] **Verify: `resolve_stage_info` exists and returns Result**
  Run: Read `crates/kirin-interpreter/src/stage_access.rs` lines 47-68
  Expected: `resolve_stage_info` returns `Result<&'ir StageInfo<L>, InterpreterError>`

- [ ] **Verify: existing tests pass**
  Run: `cargo nextest run -p kirin-interpreter`
  Expected: All tests pass

## Regression Test (P0/P1 findings)

- [ ] **Write test for P1-12: try_in_stage returns error on misconfigured pipeline**
  Create a test that:
  1. Creates an interpreter with a pipeline that does NOT have a `StageInfo<L>` for some dialect `L`
  2. Calls `try_in_stage::<L>()`
  3. Asserts it returns `Err(InterpreterError::StageResolution { ... })`

  Test file: `crates/kirin-interpreter/src/stage_access.rs` inline test, or a test module.

- [ ] **Run the test**
  Run: `cargo nextest run -p kirin-interpreter -E 'test(try_in_stage)'`
  Expected: PASS (tests the new API directly)

## Implementation Steps

- [ ] **Step 1: Add `try_in_stage` to `StageAccess`**
  In `crates/kirin-interpreter/src/stage_access.rs`, add a `try_in_stage` method:
  ```rust
  fn try_in_stage<L>(&mut self) -> Result<Staged<'_, 'ir, Self, L>, InterpreterError>
  where
      Self::StageInfo: HasStageInfo<L>,
      L: Dialect,
  {
      let stage = self.resolve_stage::<L>()?;
      Ok(Staged { interp: self, stage })
  }
  ```
  Keep `in_stage()` unchanged (it panics, as intended per user decision: provide both).

- [ ] **Step 2: Export `try_in_stage` in the prelude if appropriate**
  Check if `StageAccess` is already in the prelude. If so, `try_in_stage` is automatically available.

- [ ] **Step 3: Replace Vec collect with zip in bind_block_args**
  In `crates/kirin-interpreter/src/block_eval.rs`, replace:
  ```rust
  let arg_ssas: Vec<SSAValue> = block_info.arguments.iter().map(|ba| SSAValue::from(*ba)).collect();
  for (ssa, value) in arg_ssas.into_iter().zip(args.into_iter()) { ... }
  ```
  with direct iteration:
  ```rust
  for (ba, value) in block_info.arguments.iter().zip(args.into_iter()) {
      let ssa = SSAValue::from(*ba);
      ...
  }
  ```

- [ ] **Step 4: Same optimization in propagate_block_args**
  In `crates/kirin-interpreter/src/abstract_interp/fixpoint.rs`, apply the same zip pattern.

- [ ] **Step 5: Consume frame instead of cloning in run_forward**
  In `fixpoint.rs`, after the worklist drains, pop the frame and destructure it to move the maps rather than cloning. Check if `Frame` has an `into_parts()` method or add one.

- [ ] **Step 6: Move crate-level allows to specific items**
  In `lib.rs`, remove the crate-level `#![allow(...)]`. Add targeted `#[allow(...)]` on the specific items that need them (dispatch module for `type_complexity`, trait impl blocks for bound-related lints). Run clippy to find exactly which items need the allows.

- [ ] **Step 7: Add documentation**
  - Add doc comment on `FrameStack::read` explaining it only reads from the top frame
  - Add comment at `fixpoint.rs:274` explaining why `Call` is a no-op in `propagate_control`

- [ ] **Step 8: Run all tests**
  Run: `cargo nextest run -p kirin-interpreter`
  Expected: All tests pass

- [ ] **Step 9: Run clippy**
  Run: `cargo clippy -p kirin-interpreter`
  Expected: No warnings

## Must Not Do

- Do NOT remove `in_stage()` -- keep the panicking version (user decision: provide both).
- Do NOT change `active_stage_info` to return Result -- it's the panicking version, which is intentional.
- Do NOT modify the `Interpreter` blanket trait -- it has no methods of its own.
- Do NOT introduce `#[allow(...)]` on new code. Only relocate existing allows from crate-level to item-level.
- No unsafe code.

## Validation

**Per-step checks:**
- After step 1: `cargo check -p kirin-interpreter` -- Expected: compiles
- After step 3-5: `cargo check -p kirin-interpreter` -- Expected: compiles
- After step 6: `cargo clippy -p kirin-interpreter` -- Expected: no warnings

**Final checks:**
```bash
cargo clippy -p kirin-interpreter            # Expected: no warnings
cargo nextest run -p kirin-interpreter       # Expected: all tests pass
cargo nextest run --workspace                # Expected: no regressions
cargo test --doc -p kirin-interpreter        # Expected: all doctests pass
```

**Snapshot tests:** No snapshot tests expected.

## Success Criteria

1. `try_in_stage()` returns `Result` for misconfigured pipelines instead of panicking.
2. `in_stage()` still panics (unchanged, per user decision).
3. Vec allocations in hot paths (bind_block_args, propagate_block_args) are eliminated.
4. Frame cloning at analysis completion is eliminated.
5. Crate-level lint suppressions are narrowed to specific items.
6. Documentation gaps are filled.
7. No regressions.

**Is this a workaround or a real fix?**
This is the real fix for P1-12: adding a fallible API alongside the panicking one. The performance improvements are genuine optimizations. The lint relocation is cleanup.
