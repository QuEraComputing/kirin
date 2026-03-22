# Low-Hanging Fruit

**Review report:** `docs/review/2026-03-22/report.md`
**Execution:** Single agent, sequential, review changes after completion.
**Estimated total effort:** ~3 hours

---

## Items

### LHF-1: Add `#[must_use]` to `Continuation` (P1-15)

**Issue:** `Continuation<V, Ext>` is the critical control flow return type from `interpret()`. Silently discarding it skips jumps, returns, or calls with no compiler warning.
**Crate:** kirin-interpreter | **File:** `crates/kirin-interpreter/src/control.rs:18`

**Change:**
Add `#[must_use = "continuations must be handled to advance interpreter state"]` to the `Continuation` enum definition.

**Validation:**
```bash
cargo clippy -p kirin-interpreter
cargo nextest run -p kirin-interpreter
```

---

### LHF-2: Add `default-features = false` to bat dependency (P1-16)

**Issue:** The `bat` dependency pulls ~313 transitive deps via its default features (application, git, bugreport), none of which are needed by this crate.
**Crate:** kirin-prettyless | **File:** `crates/kirin-prettyless/Cargo.toml:7`

**Change:**
Change `bat = { version = "0.26", optional = true }` to `bat = { version = "0.26", optional = true, default-features = false }`.

**Validation:**
```bash
cargo check -p kirin-prettyless --features bat
cargo nextest run -p kirin-prettyless --features bat
```

**Must not do:** Do not remove the `optional = true` flag. Do not add feature flags unless the `--features bat` build fails without them.

---

### LHF-3: Add interval_div and interval_rem re-exports (P1-22)

**Issue:** `interval_div` and `interval_rem` are public in the `interval` submodule but missing from the crate root re-exports, forcing users to use leaked internal paths.
**Crate:** kirin-interval | **File:** `crates/kirin-interval/src/lib.rs:5`

**Change:**
Add `interval_div` and `interval_rem` to the existing `pub use interval::{...}` line in `lib.rs`.

**Validation:**
```bash
cargo check -p kirin-interval
cargo nextest run -p kirin-interval
```

---

### LHF-4: Clear terminator cache in Statement::detach (P1-1)

**Issue:** After detaching a terminator statement, `BlockInfo::terminator` still points to the orphaned statement ID. Subsequent queries trusting the terminator cache observe stale data.
**Crate:** kirin-ir | **File:** `crates/kirin-ir/src/detach.rs:13`

**Change:**
In `Statement::detach`, when `parent == Some(StatementParent::Block(block))`, add a check: if `parent_info.terminator == Some(*self)`, clear it to `None`. Place this check after obtaining `parent_info` and before the linked-list pointer updates.

**Validation:**
```bash
cargo clippy -p kirin-ir
cargo nextest run -p kirin-ir
```

---

### LHF-5: Use checked_sub for linked list length decrement (P1-2)

**Issue:** Length decrement in detach is guarded only by `debug_assert`, allowing `usize` wrapping to `usize::MAX` in release builds on invariant violation.
**Crate:** kirin-ir | **File:** `crates/kirin-ir/src/detach.rs:53-57`

**Change:**
Replace the pattern:
```rust
debug_assert!(parent_info.get_len() > 0);
*parent_info.get_len_mut() -= 1;
```
with:
```rust
*parent_info.get_len_mut() = parent_info.get_len().checked_sub(1).expect("linked list length underflow: detaching from a parent with zero length");
```

Apply this to all length decrements in the detach module (both Statement::detach and the impl_detach! macro).

**Validation:**
```bash
cargo clippy -p kirin-ir
cargo nextest run -p kirin-ir
```

---

### LHF-6: Restrict Arena::gc() visibility (P1-3)

**Issue:** `gc()` is `pub` on `Arena`. All previously obtained IDs become stale with no generation counter or runtime detection. Making it `pub(crate)` prevents accidental external use.
**Crate:** kirin-ir | **File:** `crates/kirin-ir/src/arena/gc.rs:27`

**Change:**
Change `pub fn gc(...)` to `pub(crate) fn gc(...)` on `Arena::gc()`.

**Validation:**
```bash
cargo build --workspace
cargo nextest run --workspace
```

**Must not do:** Do not add generation counters or a new compaction API in this item -- that is future work. Only restrict visibility.

---

### LHF-7: Refactor print_ports to use print_port_list method (P1-18)

**Issue:** The inline closure `print_port_list` inside `print_ports()` duplicates the `print_port_list` method at lines 522-533.
**Crate:** kirin-prettyless | **File:** `crates/kirin-prettyless/src/document/ir_render.rs:164-175`

**Change:**
Replace the inline closure in `print_ports` with a call to `self.print_port_list(edge_ports)`. Verify the call produces the same document structure (comma-separated, parenthesized).

**Validation:**
```bash
cargo clippy -p kirin-prettyless
cargo nextest run -p kirin-prettyless
cargo nextest run --workspace
```

---

### LHF-8: Remove SparseHint unnecessary Clone bounds (P2 - U1 finding 6)

**Issue:** `SparseHint`'s `Index` and `IndexMut` impls require `T: Clone` unnecessarily. `DenseHint` does not have this extra bound.
**Crate:** kirin-ir | **File:** `crates/kirin-ir/src/arena/hint/sparse.rs:52-55`

**Change:**
Remove the `T: Clone` bound from the `Index<I>` and `IndexMut<I>` impls on `SparseHint<I, T>`.

**Validation:**
```bash
cargo clippy -p kirin-ir
cargo nextest run -p kirin-ir
```

---

### LHF-9: Add #[must_use] to FunctionRenderBuilder and PipelineRenderBuilder (P2 - U5)

**Issue:** These builder types lack `#[must_use]`, meaning callers can silently discard the builder without consuming it.
**Crate:** kirin-prettyless | **File:** `crates/kirin-prettyless/src/pipeline.rs:123, 164`

**Change:**
Add `#[must_use = "call .into_string(), .print(), or .bat() to produce output"]` to both `FunctionRenderBuilder` and `PipelineRenderBuilder` struct definitions.

**Validation:**
```bash
cargo clippy -p kirin-prettyless
cargo nextest run -p kirin-prettyless
```

---

### LHF-10: Add #[must_use] to PipelineDocument (P2 - U5)

**Issue:** `PipelineDocument` constructed via `new()` is only useful when `render_function()` is called, but has no `#[must_use]`.
**Crate:** kirin-prettyless | **File:** `crates/kirin-prettyless/src/pipeline.rs:83`

**Change:**
Add `#[must_use]` to `PipelineDocument`.

**Validation:**
```bash
cargo clippy -p kirin-prettyless
```

---

### LHF-11: Add #[must_use] to error types in kirin-chumsky (P2 - U2 finding 9)

**Issue:** `ParseError`, `EmitError`, `ChumskyError`, `FunctionParseError` have no `#[must_use]`, meaning callers can silently discard parse errors.
**Crate:** kirin-chumsky | **Files:** Error type definitions across the crate

**Change:**
Add `#[must_use]` to `ParseError`, `EmitError`, `ChumskyError`, and `FunctionParseError`.

**Validation:**
```bash
cargo clippy -p kirin-chumsky
cargo nextest run -p kirin-chumsky
```

---

### LHF-12: Add #[must_use] to kirin-ir arena and builder methods (P2 - U1 finding 9)

**Issue:** `Arena::alloc()`, `Arena::delete()`, `Arena::gc()`, `Pipeline::function()`, `BuilderStageInfo::finalize()`, and similar methods have no `#[must_use]`.
**Crate:** kirin-ir | **Files:** arena, pipeline, builder modules

**Change:**
Add `#[must_use]` to:
- `Arena::alloc()`, `Arena::alloc_with_id()` (returns ID)
- `Arena::delete()` (returns bool)
- `Arena::gc()` (returns IdMap)
- `Pipeline::function()`, `Pipeline::staged_function()`, `Pipeline::define_function()` (return Result)
- `BuilderStageInfo::finalize()` (returns Result)

**Validation:**
```bash
cargo clippy -p kirin-ir
cargo nextest run -p kirin-ir
cargo build --workspace
```

---

### LHF-13: Remove debug println! from test code (P3 - U8)

**Issue:** Debug `println!` statements left in test code produce noise when running the test suite.
**Crate:** tests (workspace) | **Files:** `tests/simple.rs:63`, `tests/roundtrip/composite.rs:258`

**Change:**
Remove both `println!` calls.

**Validation:**
```bash
cargo nextest run -p kirin --test simple --test composite
```

---

### LHF-14: Remove dead strip_trailing_whitespace in composite.rs (P2 - U8)

**Issue:** `strip_trailing_whitespace` is defined with `#[allow(dead_code)]` but never called.
**Crate:** tests (workspace) | **File:** `tests/roundtrip/composite.rs:8`

**Change:**
Remove the `strip_trailing_whitespace` function definition and the `#[allow(dead_code)]` annotation.

**Validation:**
```bash
cargo nextest run --test composite
```

---

### LHF-15: Handle unused Result values from register_ssa (P2 - U8)

**Issue:** Three calls to `emit_ctx.register_ssa(...)` ignore the returned `Result`, silently swallowing potential failures.
**Crate:** tests (workspace) | **File:** `tests/roundtrip/composite.rs:53-54, 143`

**Change:**
Add `.expect("register should succeed")` to each `register_ssa` call.

**Validation:**
```bash
cargo nextest run --test composite
```

---

### LHF-16: Remove unused PrettyPrintExt imports in digraph.rs (P2 - U8)

**Issue:** Five test functions import `kirin_prettyless::PrettyPrintExt` but never use it.
**Crate:** tests (workspace) | **File:** `tests/roundtrip/digraph.rs:127, 218, 282, 346, 434`

**Change:**
Remove all five `use kirin_prettyless::PrettyPrintExt;` lines.

**Validation:**
```bash
cargo nextest run --test digraph
```

---

### LHF-17: Upgrade debug_assert_eq to assert_eq in AnalysisResult::is_subseteq (P2 - U4)

**Issue:** Block argument length check uses `debug_assert_eq` which disappears in release builds. False convergence in abstract interpretation would be a silent soundness issue.
**Crate:** kirin-interpreter | **File:** `crates/kirin-interpreter/src/result.rs:103`

**Change:**
Replace `debug_assert_eq!(self_args.len(), other_args.len())` with `assert_eq!(self_args.len(), other_args.len(), "block argument count mismatch in is_subseteq")`.

**Validation:**
```bash
cargo clippy -p kirin-interpreter
cargo nextest run -p kirin-interpreter
```

---

## Execution Order

Execute items in the order listed. Each item is independent, but the order minimizes churn:
1. LHF-1 through LHF-3: Single-line additions, zero risk
2. LHF-4 through LHF-6: Core IR changes (same crate, related files)
3. LHF-7 through LHF-8: Printer dedup, SparseHint bounds
4. LHF-9 through LHF-10: Printer #[must_use] additions
5. LHF-11 through LHF-12: #[must_use] additions across crates
6. LHF-13 through LHF-16: Test cleanup
7. LHF-17: Interpreter hardening

If any item fails validation, stop and report before continuing -- do not skip items.

## Clippy and Warnings Policy

Do NOT introduce `#[allow(...)]` annotations to suppress warnings -- fix the underlying cause. Do NOT use workarounds (renaming to `_var`, dead code annotations). If a suppression seems genuinely necessary, stop and report to the lead with the root cause explanation.

## Final Validation

After all items:
```bash
cargo clippy --workspace                  # must be warning-free
cargo nextest run --workspace
cargo test --doc --workspace
```

No snapshot test changes unless explicitly expected (if so, run `cargo insta test` and report changes).

## Success Criteria

All items pass their individual validation commands. Final workspace clippy and tests pass. Each change is minimal and self-evident -- no design decisions were required.
