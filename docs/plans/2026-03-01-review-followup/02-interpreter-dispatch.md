# Plan 2: Interpreter Dispatch Simplification

**Crates**: `kirin-interpreter`, `kirin-derive-interpreter`
**Review source**: interp-critic, interp-simplifier, 4 reviewers (stage resolution)

## Goal

Eliminate duplicated code paths in the interpreter dispatch system, fix the `call_handler` panic, and add a stage resolution helper that saves ~15 lines per dialect interpret impl.

## Changes

### Phase 1: Correctness (P0)

1. **Replace `call_handler` panic with error return** (`abstract_interp/interp.rs:47-54`)
   - The `Option<fn(...)>` with `.expect()` should be initialized to a stub returning `Err(InterpreterError::custom(...))`
   - No API change — just changes the failure mode from panic to error

### Phase 2: Dispatch Unification (P1, ~100 lines removed)

2. **Unify `run_nested_calls` / `run_nested_calls_cached`** (`stack/exec.rs:142-242`)
   - These differ only in dispatch resolution method
   - Collapse into single method with a closure parameter for dispatch lookup
   - Remove the non-cached variant entirely (dispatch table is always pre-built in `new()`)

3. **Unify `push_call_frame_with_stage` / `push_call_frame_with_stage_cached`** (`stack/call.rs:61-151`)
   - Same pattern — differ only in dispatch resolution
   - Collapse with closure parameter

4. **Deduplicate `resolve_dispatch_for_stage` / `lookup_dispatch_cached`** (`stack/transition.rs:28-83`)
   - Identical logic, merge into single function

5. **Replace `HashSet<Statement>` with `FxHashSet<Statement>`** (`stack/interp.rs:51`)
   - Add `rustc-hash` dependency

### Phase 3: Stage Resolution Helper (P1, ~15 lines saved per dialect)

6. **Add `resolve_stage` helper** on `StageAccess` or as free function
   - Collapses the 8-line `active_stage() → stage() → try_stage_info()` chain
   - Signature: `fn resolve_stage<L>(&self) -> Result<&'ir StageInfo<L>, InterpreterError>`
   - Used by every dialect's `Interpretable::interpret` impl

### Phase 4: Derive Deduplication (P1)

7. **Deduplicate `build_pattern`** in kirin-derive-interpreter
   - Character-for-character identical in `interpretable/scan.rs:41-63` and `eval_call/scan.rs:46-68`
   - Extract to shared module within kirin-derive-interpreter

8. **Document `#[callable]` / `#[wraps]` interaction** for CallSemantics derivation

## Files Touched

- `crates/kirin-interpreter/Cargo.toml` (add rustc-hash)
- `crates/kirin-interpreter/src/abstract_interp/interp.rs`
- `crates/kirin-interpreter/src/stack/exec.rs`
- `crates/kirin-interpreter/src/stack/call.rs`
- `crates/kirin-interpreter/src/stack/transition.rs`
- `crates/kirin-interpreter/src/stack/interp.rs`
- `crates/kirin-interpreter/src/traits.rs` (resolve_stage helper)
- `crates/kirin-derive-interpreter/src/interpretable/scan.rs`
- `crates/kirin-derive-interpreter/src/eval_call/scan.rs`

## Validation

```bash
cargo nextest run -p kirin-interpreter
cargo nextest run -p kirin-derive-interpreter
cargo nextest run --workspace  # dialect interpret impls depend on this
```

## Recommended Skills & Workflow

**Setup**: `/using-git-worktrees` — isolate in a worktree off `main`

**Phase 1 (Correctness)**: Bug fix — use debugging-first approach.
- `/systematic-debugging` to verify the `call_handler` panic is reachable and understand the call path
- `/test-driven-development` — write a test that triggers the panic, then fix to return error

**Phase 2 (Dispatch Unification)**: Major refactoring — needs careful design.
- `/brainstorming` before unifying cached/non-cached paths — confirm dispatch table is always pre-built, decide closure vs trait object vs enum strategy for the dispatch parameter
- `/test-driven-development` — ensure existing dispatch tests pass before and after unification
- `/simplify` after each merge (exec.rs, call.rs, transition.rs) to clean up any leftover artifacts

**Phase 3 (Stage Resolution Helper)**: New API surface.
- `/brainstorming` for the `resolve_stage` helper — method on `StageAccess` vs provided method on `Interpreter` vs free function; error type design
- `/test-driven-development` — write tests for the helper, then implement

**Phase 4 (Derive Deduplication)**: Mechanical refactoring.
- `/simplify` after extracting shared `build_pattern`

**Parallelization**: `/subagent-driven-development` — Phase 1 and Phase 4 are independent of Phases 2-3 and can run in parallel

**Completion**:
- `/verification-before-completion` — full workspace tests (dialect impls depend on interpreter)
- `/requesting-code-review` before merge
- `/finishing-a-development-branch`

## Non-Goals

- Collapsing the trait hierarchy (decision: keep decomposed)
- Changing `Continuation<V, Ext>` design
- Modifying StackInterpreter type parameter defaults
- Flattening `abstract_interp/stage.rs` (trivial, can be done anytime)
