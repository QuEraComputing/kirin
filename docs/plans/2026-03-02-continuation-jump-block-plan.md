# Continuation::Jump Block Migration — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Change `Continuation::Jump` and `Fork` from `Successor` to `Block` so the interpreter operates on blocks directly.

**Architecture:** Modify the `Continuation` enum in kirin-interpreter, then update all consumers: dialect interpreter impls (kirin-cf, kirin-scf, kirin-function), interpreter internals (stack transitions, abstract interpreter, call frames), and test fixtures.

**Tech Stack:** Rust, kirin workspace crates

**Design doc:** `docs/plans/2026-03-02-continuation-jump-block-design.md`

---

### Task 1: Change Continuation enum

**Files:**
- Modify: `crates/kirin-interpreter/src/control.rs`

**Step 1: Update the Continuation enum and imports**

Replace the `Successor` import and usages in `control.rs`:

```rust
// Line 3: change import
// Before:
use kirin_ir::{CompileStage, ResultValue, SpecializedFunction, Successor};
// After:
use kirin_ir::{Block, CompileStage, ResultValue, SpecializedFunction};
```

Update the two variants in the `Continuation` enum:

```rust
// Line 22 — before:
    Jump(Successor, Args<V>),
// After:
    Jump(Block, Args<V>),

// Line 33 — before:
    Fork(SmallVec<[(Successor, Args<V>); 2]>),
// After:
    Fork(SmallVec<[(Block, Args<V>); 2]>),
```

**Step 2: Run cargo check on kirin-interpreter**

Run: `cargo check -p kirin-interpreter 2>&1 | head -80`

Expected: Compilation errors in files that construct or pattern-match Jump/Fork with Successor. This confirms the type change propagated.

**Step 3: Commit**

```
refactor(interpreter): change Continuation::Jump/Fork from Successor to Block
```

---

### Task 2: Update interpreter internals

**Files:**
- Modify: `crates/kirin-interpreter/src/stack/transition.rs`
- Modify: `crates/kirin-interpreter/src/stack/call.rs`
- Modify: `crates/kirin-interpreter/src/abstract_interp/fixpoint.rs`

**Step 1: Update stack/transition.rs**

In `advance_frame_with_stage` (line 90-93), the `Jump` arm currently calls `succ.target()`. Since it's now a `Block`, use it directly:

```rust
// Before (lines 90-93):
            Continuation::Jump(succ, args) => {
                self.bind_block_args(stage, succ.target(), args)?;
                let first = succ.target().first_statement(stage);
                self.set_current_cursor(first)?;
            }
// After:
            Continuation::Jump(block, args) => {
                self.bind_block_args(stage, *block, args)?;
                let first = block.first_statement(stage);
                self.set_current_cursor(first)?;
            }
```

**Step 2: Update stack/call.rs**

In `push_call_frame_with_stage` (line 84-85), the match arm extracts the block from Jump. Since it's now a Block directly:

```rust
// Before (line 85):
            Ok(Continuation::Jump(succ, _)) => succ.target(),
// After:
            Ok(Continuation::Jump(entry, _)) => entry,
```

**Step 3: Update abstract_interp/fixpoint.rs**

In `propagate_control` (lines 253-259), remove `.target()` calls:

```rust
// Before (lines 253-259):
            Continuation::Jump(succ, args) => {
                changed |= self.propagate_edge::<L>(stage, succ.target(), args, narrowing)?;
            }
            Continuation::Fork(targets) => {
                for (succ, args) in targets {
                    changed |= self.propagate_edge::<L>(stage, succ.target(), args, narrowing)?;
                }
            }
// After:
            Continuation::Jump(block, args) => {
                changed |= self.propagate_edge::<L>(stage, *block, args, narrowing)?;
            }
            Continuation::Fork(targets) => {
                for (block, args) in targets {
                    changed |= self.propagate_edge::<L>(stage, *block, args, narrowing)?;
                }
            }
```

**Step 4: Run cargo check on kirin-interpreter**

Run: `cargo check -p kirin-interpreter 2>&1 | head -80`

Expected: kirin-interpreter itself compiles. Errors may remain in downstream crates (dialect impls).

**Step 5: Commit**

```
refactor(interpreter): update internals for Block-based Jump/Fork
```

---

### Task 3: Update kirin-cf interpreter impl

**Files:**
- Modify: `crates/kirin-cf/src/interpret_impl.rs`

**Step 1: Update Branch and ConditionalBranch**

In the `Branch` arm (line 21), add `.target()` since the field `target` is still a `Successor`:

```rust
// Before (line 21):
                Ok(Continuation::Jump(*target, values))
// After:
                Ok(Continuation::Jump(target.target(), values))
```

In the `ConditionalBranch` arm, update all three cases (lines 39, 43, 49-52):

```rust
// Before (line 39):
                        Ok(Continuation::Jump(*true_target, values))
// After:
                        Ok(Continuation::Jump(true_target.target(), values))

// Before (line 43):
                        Ok(Continuation::Jump(*false_target, values))
// After:
                        Ok(Continuation::Jump(false_target.target(), values))

// Before (lines 49-52):
                        Ok(Continuation::Fork(smallvec![
                            (*true_target, t_values),
                            (*false_target, f_values),
                        ]))
// After:
                        Ok(Continuation::Fork(smallvec![
                            (true_target.target(), t_values),
                            (false_target.target(), f_values),
                        ]))
```

**Step 2: Remove unused Successor import if present**

The file currently imports from `kirin::prelude::{Dialect, SSAValue}`. If `Successor` is not in the import list, no change needed.

**Step 3: Run cargo check**

Run: `cargo check -p kirin-cf --features interpret 2>&1 | head -40`

Expected: PASS

**Step 4: Commit**

```
refactor(cf): use Block in Continuation::Jump/Fork
```

---

### Task 4: Update kirin-scf interpreter impl

**Files:**
- Modify: `crates/kirin-scf/src/interpret_impl.rs`

**Step 1: Update If operation**

Remove the `Successor::from_block()` conversions and use `Block` directly (lines 36-46):

```rust
// Before (lines 36-46):
        let then_target = Successor::from_block(self.then_body);
        let else_target = Successor::from_block(self.else_body);
        match cond.is_truthy() {
            Some(true) => Ok(Continuation::Jump(then_target, smallvec![])),
            Some(false) => Ok(Continuation::Jump(else_target, smallvec![])),
            None => Ok(Continuation::Fork(smallvec![
                (then_target, smallvec![]),
                (else_target, smallvec![]),
            ])),
        }
// After:
        match cond.is_truthy() {
            Some(true) => Ok(Continuation::Jump(self.then_body, smallvec![])),
            Some(false) => Ok(Continuation::Jump(self.else_body, smallvec![])),
            None => Ok(Continuation::Fork(smallvec![
                (self.then_body, smallvec![]),
                (self.else_body, smallvec![]),
            ])),
        }
```

**Step 2: Remove unused Successor import**

```rust
// Before (line 1):
use kirin::prelude::{CompileTimeValue, Dialect, HasStageInfo, Successor};
// After:
use kirin::prelude::{CompileTimeValue, Dialect, HasStageInfo};
```

**Step 3: Run cargo check**

Run: `cargo check -p kirin-scf --features interpret 2>&1 | head -40`

Expected: PASS

**Step 4: Commit**

```
refactor(scf): use Block directly in Continuation::Jump/Fork
```

---

### Task 5: Update kirin-function interpreter impl

**Files:**
- Modify: `crates/kirin-function/src/interpret_impl.rs`

**Step 1: Update FunctionBody::interpret**

Replace `Successor::from_block(entry)` with `entry` directly (lines 39-42):

```rust
// Before (lines 39-42):
        Ok(Continuation::Jump(
            kirin::prelude::Successor::from_block(entry),
            smallvec![],
        ))
// After:
        Ok(Continuation::Jump(entry, smallvec![]))
```

**Step 2: Run cargo check**

Run: `cargo check -p kirin-function --features interpret 2>&1 | head -40`

Expected: PASS

**Step 3: Commit**

```
refactor(function): use Block directly in Continuation::Jump
```

---

### Task 6: Update test fixtures

**Files:**
- Modify: `crates/kirin-test-utils/src/ir_fixtures.rs`
- Modify: `crates/kirin-interpreter/tests/derive_macros.rs`
- Modify: `crates/kirin-interpreter/tests/stage_dispatch.rs`

**Step 1: Update kirin-test-utils/src/ir_fixtures.rs**

This file has ~11 uses of `Successor::from_block()` when building ControlFlow operations. These are constructing dialect ops (not Continuation), so they stay as `Successor::from_block()` — **no changes needed here**. The `Branch` and `ConditionalBranch` struct fields are still `Successor` type.

Verify: read the file and confirm all `Successor::from_block()` calls are constructing `ControlFlow` dialect operations, not `Continuation` variants.

**Step 2: Update kirin-interpreter/tests/derive_macros.rs**

Same situation — `Successor::from_block()` is used to build `ControlFlow` dialect ops, not `Continuation`. **No changes needed.**

**Step 3: Update kirin-interpreter/tests/stage_dispatch.rs**

Same — `Successor::from_block()` constructs dialect ops. **No changes needed.**

**Step 4: Run full test suite**

Run: `cargo nextest run --workspace 2>&1 | tail -30`

Expected: All tests pass.

**Step 5: Run doctests**

Run: `cargo test --doc --workspace 2>&1 | tail -20`

Expected: All doctests pass.

**Step 6: Commit (if any changes were needed)**

```
test: verify test suite passes with Block-based Jump/Fork
```

---

### Task 7: Final validation and cleanup

**Step 1: Run cargo fmt**

Run: `cargo fmt --all`

**Step 2: Search for any remaining Successor references in interpreter context**

Run a grep for `Successor` in kirin-interpreter to confirm no stale references remain:

Search: `Successor` in `crates/kirin-interpreter/src/**/*.rs`

Expected: Zero matches (Successor is no longer imported or used in interpreter code).

**Step 3: Run full workspace build + test**

Run: `cargo build --workspace && cargo nextest run --workspace`

Expected: All green.

**Step 4: Commit any formatting changes**

```
chore: format code
```
