# New Derive Package For Interpreter-2

**Wave:** 4
**Agent role:** Implementer
**Estimated effort:** large

---

## Issue

`kirin-interpreter-2` should not depend on the old derive crate as a migration
bridge. The new interpreter crate has a different execution model, different
runtime traits, and different effect protocol. That means it needs its own
derive package designed around those v2 contracts.

This wave creates and stabilizes a new derive crate,
`kirin-derive-interpreter-2`, before any downstream dialect migration starts.

## Scope

**Files to add or modify:**

- `Cargo.toml`
- `crates/kirin-derive-interpreter-2/Cargo.toml`
- `crates/kirin-derive-interpreter-2/src/*`
- `crates/kirin-interpreter-2/Cargo.toml`
- `crates/kirin-interpreter-2/src/lib.rs`
- tests for the new derive crate, including `trybuild` coverage if used

**Out of scope:**

- downstream dialect migration,
- deleting or rewriting `kirin-derive-interpreter`,
- default-switch plumbing.

## Design Goals

- Leave `kirin-derive-interpreter` intact for the old interpreter crate.
- Design the new macro surface around `kirin-interpreter-2`, not around
  continuation-era compatibility shims.
- Freeze the v2 derive surface before any pilot dialect crate starts
  migrating.

## Expected Macro Surface

The exact names should be decided in this wave, but the package should support
the v2 equivalents of:

- statement interpretation derive support,
- callable-body derive support for standard callable body shapes,
- any compatibility helper derives that are needed for the approved v2 runtime
  surface.

If the final macro names differ from the old package, document that explicitly.

## Implementation Steps

- [ ] Add `crates/kirin-derive-interpreter-2` as a new workspace member and
  workspace dependency.
- [ ] Reuse shared derive infrastructure from `kirin-derive-toolkit` where that
  is actually common, but do not force the v2 package to preserve old macro
  contracts that no longer fit.
- [ ] Design the v2 derive API explicitly:
  choose macro names,
  choose helper attributes,
  define crate-path override behavior for `::kirin_interpreter_2`.
- [ ] Implement the new derives against the v2 trait contracts and effect
  return types.
- [ ] Add focused tests proving the derives generate usable code for
  `kirin-interpreter-2`.
- [ ] Add documentation or crate-level examples that downstream dialect authors
  can follow during migration.

## Validation

Run:

```bash
cargo build -p kirin-derive-interpreter-2
cargo nextest run -p kirin-derive-interpreter-2
```

If the crate uses `trybuild`, include its test suite here as well.

## Success Criteria

1. `kirin-derive-interpreter-2` exists as a separate crate in the workspace.
2. The derive surface is designed around the v2 runtime rather than around the
   old crate's abstractions.
3. The new derive package is test-covered and stable enough to be a prerequisite
   for downstream migration.
4. No downstream dialect crate has to mix old and new derive packages while the
   new derive API is still unsettled.
