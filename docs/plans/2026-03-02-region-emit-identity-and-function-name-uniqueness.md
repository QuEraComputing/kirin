# Region Emit Identity & Function Name Uniqueness Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Fix the two-pass `Region::emit` identity bug (stale parent/block-arg IDs after stub swap), forbid duplicate abstract function names so `function_by_name` is deterministic by construction, and update the review report to match the current interface decisions.

**Architecture:** Keep the two-pass parser strategy (stub registration first, emit bodies second), but move identity remapping into `kirin-ir` where private IR internals are accessible. Enforce function-name uniqueness at allocation time in `Pipeline::function()` to preserve existing API shape while eliminating last-write-wins behavior in `name_index`.

**Tech Stack:** Rust 2024, `kirin-chumsky`, `kirin-ir`, `kirin-function`, `cargo nextest`, `cargo fmt`.

---

### Task 1: Add Failing Regression Tests for Two-Pass Region Identity

**Files:**
- Modify: `crates/kirin-chumsky-derive/tests/forward_block_ref.rs`

**Step 1: Write the failing test for statement parent identity**

Add a helper assertion and test that validates, for each block in emitted region:
- every non-terminator statement has `stmt.parent(&stage) == Some(current_block)`
- terminator (if present) also has `parent == Some(current_block)`
- the parent block is not deleted in block arena

```rust
fn assert_region_parent_identity(stage: &StageInfo<BranchLang>, body: kirin::ir::Region) {
    for block in body.blocks(stage) {
        assert!(!block.expect_info(stage).deleted(), "region block should be live");

        for stmt in block.statements(stage) {
            assert_eq!(*stmt.parent(stage), Some(block), "statement parent mismatch");
        }

        if let Some(term) = block.terminator(stage) {
            assert_eq!(*term.parent(stage), Some(block), "terminator parent mismatch");
        }
    }
}
```

**Step 2: Write the failing test for block-argument `SSAKind` identity**

Extend the same helper to validate each block argument:
- `arg.expect_info(stage).kind()` is `SSAKind::BlockArgument(block, idx)`

```rust
for (idx, arg) in block.expect_info(stage).arguments.iter().enumerate() {
    match arg.expect_info(stage).kind() {
        SSAKind::BlockArgument(owner, owner_idx) => {
            assert_eq!(*owner, block);
            assert_eq!(*owner_idx, idx);
        }
        other => panic!("expected SSAKind::BlockArgument, got {other:?}"),
    }
}
```

**Step 3: Run test to verify it fails on current code**

Run: `cargo nextest run -p kirin-chumsky-derive -E 'test(forward_block_ref)'`
Expected: at least one new identity assertion fails (parent mismatch and/or block-arg owner mismatch).

**Step 4: Commit test-only change**

```bash
git add crates/kirin-chumsky-derive/tests/forward_block_ref.rs
git commit -m "test(chumsky): add identity invariants for two-pass region emit"
```

---

### Task 2: Fix Two-Pass Region Identity Remap in `kirin-ir` and Use It from Parser

**Files:**
- Modify: `crates/kirin-ir/src/context.rs`
- Modify: `crates/kirin-chumsky/src/ast.rs`
- Modify: `crates/kirin-ir/src/context.rs` (remove now-unnecessary `block_arena_mut` if unused)

**Step 1: Add a focused `StageInfo` helper that remaps a swapped block identity atomically**

Add a method (public, small surface, documented as parser-internal behavior) that:
1. clones source (`real`) block info
2. rewrites statement parents from `real` -> `stub`
3. rewrites block-arg kinds from `SSAKind::BlockArgument(real, idx)` -> `SSAKind::BlockArgument(stub, idx)`
4. writes remapped info into `stub`, forcing `node.ptr = stub`
5. deletes `real`

```rust
pub fn remap_block_identity(&mut self, stub: Block, real: Block) {
    let mut real_info = real.expect_info(self).clone();

    // Re-parent statements
    for stmt in real.statements(self) {
        stmt.expect_info_mut(self).parent = Some(stub);
    }
    if let Some(term) = real.terminator(self) {
        term.expect_info_mut(self).parent = Some(stub);
    }

    // Rebind block-argument ownership
    for (idx, arg) in real_info.arguments.iter().copied().enumerate() {
        let arg_info = arg.expect_info_mut(self);
        if matches!(arg_info.kind, SSAKind::BlockArgument(_, _)) {
            arg_info.kind = SSAKind::BlockArgument(stub, idx);
        }
    }

    real_info.node.ptr = stub;
    *stub.expect_info_mut(self) = real_info;
    self.blocks.delete(real);
}
```

**Step 2: Replace direct stub-swap logic in `Region::emit` with the helper**

In `crates/kirin-chumsky/src/ast.rs`, replace:
- manual clone/assign/delete loop
with:

```rust
for (&stub, &real) in stub_blocks.iter().zip(real_blocks.iter()) {
    ctx.stage.remap_block_identity(stub, real);
}
```

**Step 3: Remove `block_arena_mut()` if no longer needed**

After switching to `remap_block_identity`, remove `block_arena_mut()` if unused to avoid unnecessary API expansion.

**Step 4: Run tests for parser + regression**

Run:
- `cargo nextest run -p kirin-chumsky-derive -E 'test(forward_block_ref)'`
- `cargo nextest run -p kirin-chumsky`

Expected: all pass, including new identity invariants.

**Step 5: Commit fix**

```bash
git add crates/kirin-ir/src/context.rs crates/kirin-chumsky/src/ast.rs
git commit -m "fix(chumsky): preserve block identity invariants in two-pass region emit"
```

---

### Task 3: Forbid Duplicate Abstract Function Names at Allocation Time

**Files:**
- Modify: `crates/kirin-ir/src/pipeline.rs`
- Modify: `crates/kirin-ir/src/pipeline.rs` (add `#[cfg(test)] mod tests`)
- Modify (if needed by compile fallout):
  - `crates/kirin-chumsky/src/function_text/parse_text.rs`
  - `crates/kirin-function/src/interpret_impl.rs`

**Step 1: Write failing test for duplicate name allocation**

Add test in `pipeline.rs`:

```rust
#[test]
#[should_panic(expected = "duplicate abstract function name")]
fn duplicate_function_names_are_forbidden() {
    let mut pipeline: Pipeline<()> = Pipeline::new();
    let _ = pipeline.function().name("foo").new();
    let _ = pipeline.function().name("foo").new();
}
```

Add positive test:

```rust
#[test]
fn function_by_name_is_stable_for_unique_names() {
    let mut pipeline: Pipeline<()> = Pipeline::new();
    let foo = pipeline.function().name("foo").new();
    let sym = pipeline.lookup_symbol("foo").unwrap();
    assert_eq!(pipeline.function_by_name(sym), Some(foo));
}
```

**Step 2: Run test to verify failure**

Run: `cargo nextest run -p kirin-ir -E 'test(duplicate_function_names_are_forbidden|function_by_name_is_stable_for_unique_names)'`
Expected: duplicate-name test fails before implementation.

**Step 3: Implement uniqueness check in `Pipeline::function()`**

In `pipeline.rs`, before inserting into `name_index`:

```rust
if let Some(ref n) = name {
    if let Some(sym) = self.lookup_symbol(n) {
        if self.function_by_name(sym).is_some() {
            panic!("duplicate abstract function name: {n}");
        }
    }
}
```

Then proceed to intern + allocate + insert.

**Step 4: Update docs for `Pipeline::function()` panic behavior**

In doc comments, add `# Panics` for duplicate names.

**Step 5: Run `kirin-ir` and dependent focused tests**

Run:
- `cargo nextest run -p kirin-ir`
- `cargo nextest run -p kirin-function`
- `cargo nextest run -p kirin-chumsky -E 'test(function_text)'`

Expected: all pass; duplicate names now rejected at allocation.

**Step 6: Commit fix**

```bash
git add crates/kirin-ir/src/pipeline.rs crates/kirin-chumsky/src/function_text/parse_text.rs crates/kirin-function/src/interpret_impl.rs
git commit -m "fix(ir): forbid duplicate abstract function names in pipeline"
```

---

### Task 4: Update Review Report to Match Current Decisions

**Files:**
- Modify: `docs/plans/2026-03-01-review-followup/REPORT.md`

**Step 1: Update Plan 5 status and deferred-items section**

Adjust report text to reflect current reality:
- remove stale claim that pretty-printer trait consolidation / print-surface collapse are deferred
- mark these as implemented if current code is the canonical interface decision

**Step 2: Add note about the two-pass identity follow-up fix**

In “Reviewer Observations” (or “What Was Implemented”), add explicit line that parent/block-arg identity remap is now fixed (not only `node.ptr` cosmetic discussion).

**Step 3: Validate docs formatting**

Run:
- `cargo fmt --all` (if code/doc comments changed)
- `rg -n "PrettyPrintName/PrettyPrintType consolidation|PrintExt/PipelinePrintExt collapse" docs/plans/2026-03-01-review-followup/REPORT.md`

Expected: stale deferred statements removed or rewritten.

**Step 4: Commit docs update**

```bash
git add docs/plans/2026-03-01-review-followup/REPORT.md
git commit -m "docs(reviews): align follow-up report with implemented printer and parser decisions"
```

---

### Task 5: Final Verification Before Merge Claim

**Files:**
- No new files; verification only.

**Step 1: Run full relevant test matrix**

Run:
- `cargo nextest run -p kirin-ir`
- `cargo nextest run -p kirin-chumsky`
- `cargo nextest run -p kirin-chumsky-derive`
- `cargo nextest run -p kirin-function`
- `cargo nextest run --workspace`
- `cargo test --doc --workspace`

Expected: all green.

**Step 2: Sanity-check target behaviors manually in tests**

Confirm:
- forward block refs still parse
- region block/statement/arg identity assertions pass
- duplicate function-name allocation panics with clear message
- `function_by_name` resolves uniquely allocated names

**Step 3: Prepare for review**

Collect:
- list of modified files
- concise risk notes (two-pass emit internals + function-allocation panic semantics)

