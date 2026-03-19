# Implementer -- Code Quality Review: kirin-interpreter

## Clippy Workaround Audit

No `#[allow(...)]` attributes found in any source files under `crates/kirin-interpreter/src/`. This is a clean crate from a clippy perspective.

## Logic Duplication

### 1. ValueStore and StageAccess impls for StackInterpreter and AbstractInterpreter (P3, confirmed)

**Files:** `src/stack/interp.rs` (StackInterpreter) vs `src/abstract_interp/interp.rs` (AbstractInterpreter)

Both interpreters implement `ValueStore`, `StageAccess<'ir>`, and `BlockEvaluator<'ir>` with very similar bodies:
- `ValueStore::read` delegates to `self.frames.read(value)` (Stack clones, Abstract clones)
- `ValueStore::write` delegates to `self.frames.write(result, value)`
- `ValueStore::write_ssa` delegates to `self.frames.write_ssa(ssa, value)`
- `StageAccess::pipeline` returns `self.pipeline`
- `StageAccess::active_stage` returns `self.frames.active_stage_or(self.root_stage)`

This is intentional per the trait decomposition design (each interpreter owns its fields differently), so it is not actionable. The duplication is minimal and each impl block has different trait bounds.

### 2. Constructor pattern: `new` + `new_with_global` (P3, confirmed)

**Files:** `src/stack/interp.rs:58-106` and `src/abstract_interp/interp.rs:91-125`

Both `StackInterpreter` and `AbstractInterpreter` have `new(pipeline, stage) -> Self` that delegates to `new_with_global(pipeline, stage, ())`. This is a standard Rust pattern and is not problematic.

### 3. Builder methods duplicate between interpreters (P3, confirmed)

Both interpreters have `with_max_depth`, `global()`, `global_mut()` with identical implementations. However, since they are on different concrete types with different type parameters, extraction is not straightforward. Not actionable.

## Rust Best Practices

### Missing `#[must_use]` annotations (P2, confirmed)

Zero `#[must_use]` annotations in the crate. Key candidates:
- `AnalysisResult::bottom()`, `AnalysisResult::new()` -- constructors
- `AnalysisResult::ssa_value()`, `AnalysisResult::return_value()` -- pure accessors
- `AnalysisResult::is_subseteq()` -- pure predicate
- `StackInterpreter::new()`, `AbstractInterpreter::new()` -- constructors
- `StackInterpreter::with_fuel()`, `.with_max_depth()` -- builder methods
- `FrameStack::new()`, `.with_capacity()`, `.depth()`, `.is_empty()` -- standard container methods
- `Continuation` variants -- the enum itself should probably be `#[must_use]`

### Manual Debug and Clone impls for AnalysisResult (P3, confirmed)

**File:** `src/result.rs:16-34`

`AnalysisResult<V>` has hand-written `Debug` and `Clone` impls that are identical to what `#[derive(Debug, Clone)]` would produce. The manual impls exist because the derives would require `V: Debug` and `V: Clone` unconditionally on the struct, but `Debug` already requires `V: Debug` (line 16) and `Clone` requires `V: Clone` (line 26), which is exactly what the derives would do. These can be replaced with derives.

### `.unwrap()` usage in non-test code (P2, confirmed)

**File:** `src/frame_stack.rs` -- 15 occurrences total, but 14 are in tests. The non-test code uses proper `Result`-based error handling via `ok_or_else`. Clean.

**File:** `src/abstract_interp/summary.rs` -- 5 occurrences, all in internal logic where the invariant is maintained by construction (e.g., unwrapping after a `.find()` that was just confirmed to exist). Acceptable.

**File:** `src/frame.rs` -- 4 occurrences in tests only. Clean.

### `InterpreterError` is not `Clone` (P3, confirmed)

**File:** `src/error.rs`

`InterpreterError` contains `Box<dyn Error + Send + Sync>` in the `Custom` variant, preventing `Clone`. This is intentional (supports arbitrary user errors), but it means error values cannot be cheaply duplicated for retrying or logging. The tradeoff is reasonable.

### `Continuation` lacks `Clone` (P3, confirmed)

**File:** `src/control.rs`

`Continuation<V, Ext>` does not derive `Clone` despite being an enum of clonable data (when `V: Clone` and `Ext: Clone`). Adding a conditional derive would allow cloning continuations in diagnostic/logging contexts. Currently the code uses `match &control` references to avoid this.

## Summary

- P2 confirmed -- Missing `#[must_use]` across the crate (zero instances, many pure functions)
- P3 confirmed -- `src/result.rs:16-34`: Manual Debug/Clone impls replaceable with derives
- P3 confirmed -- `src/control.rs`: `Continuation<V, Ext>` could derive `Clone` conditionally
- P3 confirmed -- Constructor duplication across StackInterpreter/AbstractInterpreter is minimal and intentional
