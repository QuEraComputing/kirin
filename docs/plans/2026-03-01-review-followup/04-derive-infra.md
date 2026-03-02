# Plan 4: Derive Infrastructure Consolidation

**Crates**: `kirin-derive-core`, `kirin-derive-dialect`, `kirin-derive`
**Review source**: derive-critic, derive-simplifier, 3 reviewers (PhantomData)

## Goal

Reduce the derive infrastructure from 3 layers to 2, auto-inject PhantomData, fix incomplete validation, and eliminate code duplication.

## Changes

### Phase 1: Correctness (P0)

1. **Complete property lattice validation** (`property/scan.rs:40-85`)
   - Currently only validates `speculatable => pure`
   - Add `constant => pure` check
   - Emit compile error when `#[kirin(constant)]` is set without `#[kirin(pure)]`

### Phase 2: PhantomData Auto-Injection (P1, saves ~40 lines across 5 dialects)

2. **Auto-default PhantomData fields** without requiring `#[kirin(default)]`
   - The derive already detects PhantomData at `builder/helpers.rs:91`
   - When a field's type is `PhantomData<T>`, automatically treat it as defaulted
   - Remove the need for `#[kirin(default)] marker: PhantomData<T>` boilerplate
   - All 5+ dialect crates benefit

### Phase 3: Crate Merge (P1, removes 1 crate)

3. **Merge `kirin-derive-dialect` into `kirin-derive-core`**
   - The split is artificial — `kirin-derive-dialect` has exactly one consumer (`kirin-derive`)
   - Current: `kirin-derive-core` (shared IR) → `kirin-derive-dialect` (generators) → `kirin-derive` (proc-macro)
   - Target: `kirin-derive-core` (shared IR + generators) → `kirin-derive` (proc-macro entry points)
   - **Steps**:
     - Move all code generators from kirin-derive-dialect/src/ into kirin-derive-core/src/
     - Update kirin-derive to depend on kirin-derive-core only
     - Remove kirin-derive-dialect from workspace
     - Update Cargo.toml workspace members

### Phase 4: Code Deduplication (P1)

4. **Extract duplicated field iteration code**
   - `all_fields`/`field_pattern` duplicated between `field/iter/statement.rs:129-139` and `property/statement.rs:58-78`
   - `field_name_tokens` duplicated between `field/iter/helpers.rs:75-78` and `property/statement.rs:80-83`
   - Extract shared helpers

5. **Remove unused phantom lifetime** on `FieldAccess<'a>` (`field/iter/statement.rs:178`)

### Phase 5: Cleanup (P2-P3)

6. **Add `callable` to `error_unknown_attribute`** (`misc.rs:124-162`)
   - Include hint pointing to `#[derive(CallSemantics)]`

7. **Remove deprecated `InputBuilder`/`InputContext` re-exports**

8. **Rename `#[kirin(fn)]` to `#[kirin(builder)]`**
   - Decision from interview: more self-explanatory
   - Add deprecated alias for migration

## Files Touched

- `crates/kirin-derive-core/Cargo.toml` (absorb kirin-derive-dialect deps)
- `crates/kirin-derive-core/src/` (absorb generator modules)
- `crates/kirin-derive/Cargo.toml` (drop kirin-derive-dialect dep)
- `crates/kirin-derive/src/lib.rs` (update imports)
- `crates/kirin-derive-dialect/` (removed entirely)
- `Cargo.toml` (workspace members)
- `crates/kirin-derive-core/src/property/scan.rs` (lattice validation)
- `crates/kirin-derive-core/src/builder/helpers.rs` (PhantomData auto-default)
- `crates/kirin-derive-core/src/misc.rs` (error_unknown_attribute)

## Validation

```bash
cargo nextest run -p kirin-derive-core
cargo nextest run -p kirin-derive
cargo nextest run --workspace  # all crates use derive macros
cargo test --doc --workspace
```

## Recommended Skills & Workflow

**Setup**: `/using-git-worktrees` — isolate in a worktree off `main`

**Phase 1 (Correctness)**: Small, targeted fix.
- `/test-driven-development` — write a compile-fail test: `#[kirin(constant)]` without `#[kirin(pure)]` should emit an error. Then add the validation.

**Phase 2 (PhantomData Auto-Injection)**: Derive macro behavior change.
- `/test-driven-development` — write a test dialect without `#[kirin(default)]` on PhantomData, verify it compiles after the change
- `/simplify` — clean up dialects that no longer need the `#[kirin(default)]` annotation

**Phase 3 (Crate Merge)**: Structural refactoring — needs careful planning.
- `/brainstorming` before merging kirin-derive-dialect into kirin-derive-core — explore module organization in the merged crate, ensure no circular dependencies, plan the git history (one big move vs incremental)
- `/subagent-driven-development` — the actual file moves can be parallelized: one agent moves modules, another updates imports and Cargo.toml

**Phase 4-5 (Deduplication & Cleanup)**: Mechanical.
- `/simplify` after extracting shared helpers and removing deprecated re-exports

**Rename `#[kirin(fn)]` → `#[kirin(builder)]`**:
- `/test-driven-development` — test both old (deprecated) and new attribute names work

**Completion**:
- `/verification-before-completion` — full workspace tests (all crates use derive macros)
- `/requesting-code-review` — crate merge needs careful review
- `/finishing-a-development-branch`

## Non-Goals

- Unifying Scan/Emit into single DeriveVisitor (loses phase separation)
- Token builder macro (P3, high effort)
- Renaming `#[kirin(type=...)]` (decision: keep as-is)
- Reducing 3 near-identical token builders (P3)
