# Low-Hanging Fruit

**Review report:** `docs/plans/2026-03-22-full-workspace-refactor/implementation-notes.md`
**Execution:** Single agent, sequential, review changes after completion.
**Estimated total effort:** ~15 minutes

---

## Items

### LHF-1: Add Debug impl for Staged (#7)

**Issue:** `Staged<'a, 'ir, I, L>` doesn't implement `Debug`, preventing `unwrap()`/`expect()` on `Result<Staged, ...>` in tests. Tests must use `is_ok()`/`is_err()` workarounds instead.
**Crate:** kirin-interpreter | **File:** `crates/kirin-interpreter/src/stage.rs:15-18`

**Change:**
Add a manual `impl Debug for Staged` that shows only the `stage` field and skips `interp` (which holds `&mut I` with no useful Debug output). Use `finish_non_exhaustive()` to indicate hidden fields.

```rust
impl<I, L: Dialect> std::fmt::Debug for Staged<'_, '_, I, L>
where
    StageInfo<L>: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Staged")
            .field("stage", &self.stage)
            .finish_non_exhaustive()
    }
}
```

Note: `I` is declared as a generic parameter but does NOT need a `Debug` bound — we intentionally skip it. The `StageInfo<L>: Debug` bound is satisfied because `StageInfo` derives `Debug` (see `kirin-ir/src/stage/info.rs:65`).

After adding, optionally update the test at `crates/kirin-interpreter/src/stage_access.rs:181-184` (`try_in_stage_succeeds_on_valid_pipeline`) to use `unwrap()` instead of the `is_ok()` assert, demonstrating the improvement. Note: this test calls `try_in_stage` which returns `Result<Staged, ...>` — with `Debug` on `Staged`, `unwrap()` now works.

**Validation:**
```bash
cargo nextest run -p kirin-interpreter
```

**Must not do:** Do not add a `Debug` bound on `I` — the interpreter type has no meaningful Debug representation and adding the bound would cascade to all callers.

---

## Execution Order

Execute items in the order listed. Each item is independent, but the order
minimizes churn (e.g., renames before downstream changes).

If any item fails validation, stop and report before continuing — do not
skip items.

## Clippy and Warnings Policy

Do NOT introduce `#[allow(...)]` annotations to suppress warnings — fix the
underlying cause. Do NOT use workarounds (renaming to `_var`, dead code
annotations). If a suppression seems genuinely necessary, stop and report
to the lead with the root cause explanation.

## Final Validation

After all items:
```bash
cargo clippy --workspace                  # must be warning-free
cargo nextest run --workspace
cargo test --doc --workspace
```

No snapshot test changes unless explicitly expected (if so, run
`cargo insta test` and report changes).

## Success Criteria

All items pass their individual validation commands. Final workspace clippy
and tests pass. Each change is minimal and self-evident — no design decisions
were required.
