# Plan 1: IR Core Improvements

**Crates**: `kirin-ir`
**Review source**: ir-critic, ir-simplifier, dialect-critic, dialect-simplifier

## Goal

Improve kirin-ir's correctness documentation, performance, and ergonomics without changing the public trait surface.

## Changes

### Phase 1: Safety & Performance (P0 + P3)

1. **Document `Arena::gc()` safety hazard** (`arena/data.rs`)
   - Add `# Safety` section warning that all existing references become stale after gc
   - Add debug_assert or tracking mechanism for stale pointer detection (optional)
   - Decision: Keep gc() — needed for future rewrite framework

2. **Switch `InternTable` to `FxHashMap`** (`intern.rs:12`)
   - Add `rustc-hash` dependency to kirin-ir
   - Replace `std::collections::HashMap` with `FxHashMap` in InternTable
   - Free performance win, no API change

3. **Remove redundant `.map(|x| x)` in `Arena::iter()`** (`arena/data.rs:66`)

4. **Remove `Clone` bound from `DenseHint` Index impls** (`arena/hint/dense.rs:56-60`)

5. **`Detach::detach` return `()` not `eyre::Result<()>`** (`detach.rs:8`)
   - Implementation never errors; simplify the return type

### Phase 2: Ergonomics (P1 + P2)

6. **Add `Pipeline::function_by_name(&self, name: &str) -> Option<Function>`**
   - Currently `Call::interpret` does O(N) linear scan of function arena
   - Add a name→Function index (FxHashMap) to Pipeline or StageInfo
   - This is used by kirin-function's Call interpret impl and potentially other dialects

7. **Remove `PhantomData<L>` from `BlockInfo` and `RegionInfo`**
   - These contain no dialect-specific data — `L` only exists for `GetInfo<L>` routing
   - Making them non-generic eliminates dialect bounds from many downstream signatures
   - Requires updating `GetInfo` implementations to route without `L` on the info types
   - **Risk**: Medium — cascading signature changes. Prototype first.

### Phase 3: Small Wins (P3)

8. **`FxHashSet<Use>` → `SmallVec<[Use; 2]>`** for SSA use tracking
   - Most SSA values have 1-3 uses; SmallVec avoids heap allocation
   - Add `smallvec` dependency

9. **Gate `TestSSAValue` behind `#[cfg(test)]` or feature flag** (`node/ssa.rs:52`)

## Files Touched

- `crates/kirin-ir/Cargo.toml` (add rustc-hash, smallvec)
- `crates/kirin-ir/src/intern.rs`
- `crates/kirin-ir/src/arena/data.rs`
- `crates/kirin-ir/src/arena/hint/dense.rs`
- `crates/kirin-ir/src/detach.rs`
- `crates/kirin-ir/src/pipeline.rs` (function_by_name)
- `crates/kirin-ir/src/node/block.rs` (PhantomData removal)
- `crates/kirin-ir/src/node/region.rs` (PhantomData removal)
- `crates/kirin-ir/src/node/ssa.rs` (TestSSAValue gating)

## Validation

```bash
cargo nextest run -p kirin-ir
cargo nextest run --workspace  # check cascade from PhantomData removal
cargo test --doc -p kirin-ir
```

## Recommended Skills & Workflow

**Setup**: `/using-git-worktrees` — isolate in a worktree off `main`

**Phase 1 (Safety & Performance)**: These are small, independent fixes.
- `/test-driven-development` for item 5 (`Detach::detach` return type change) — write a test asserting `()` return before changing the signature
- `/systematic-debugging` if any cascade issues arise from `Detach` signature change
- `/simplify` after each batch of mechanical changes (FxHashMap swap, dead code removal)

**Phase 2 (Ergonomics)**: These involve new API surface and risky refactors.
- `/brainstorming` before `Pipeline::function_by_name()` — explore where the index lives (Pipeline vs StageInfo), lazy vs eager indexing, what happens on function rename
- `/test-driven-development` for `function_by_name` — write lookup tests before implementation
- `/brainstorming` before PhantomData removal — prototype the cascading signature changes, decide if `GetInfo` routing needs a different mechanism
- `/subagent-driven-development` to parallelize Phase 1 items (independent mechanical fixes) while brainstorming Phase 2

**Completion**:
- `/verification-before-completion` — run full workspace tests, confirm no regressions
- `/simplify` — final pass on changed files
- `/requesting-code-review` before merge
- `/finishing-a-development-branch` — decide merge/PR strategy

## Non-Goals

- Changing Arena's allocation strategy
- Modifying the Dialect trait surface
- `WithStage<'a, L>` convenience wrapper (P2, can be added later)
- `pipeline.simple_function()` shortcut (P2, additive)
