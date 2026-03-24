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
- Design the new macro surface around `kirin-interpreter-2`, not around old
  continuation-era compatibility shims.
- Freeze the v2 derive surface before any pilot dialect crate starts
  migrating.
- Land the shared `kirin-derive-toolkit` template and layout work that the new
  derive crate depends on before downstream migration starts.

## Expected Macro Surface

- `#[derive(Interpretable)]`
- `#[derive(ConsumeResult)]`
- `#[derive(CallableBody)]`
- `#[derive(SSACFGCallableBody)]`

Helper attributes:

- reuse `#[wraps]`
- reuse `#[callable]`
- reuse `#[interpret(...)]` for interpreter crate path override
- add `#[body]` for concrete body derives

## Implementation Steps

- [ ] Add `crates/kirin-derive-interpreter-2` as a new workspace member and
  workspace dependency.
- [ ] Add the required shared support in `kirin-derive-toolkit`:
  wrapper-forwarding templates,
  selector-policy configuration,
  `#[body]` field lookup and validation,
  and an interpreter-oriented layout/context path for normalized
  `#[interpret(...)]`, `#[callable]`, and `#[body]` metadata.
- [ ] Implement the approved v2 derive API explicitly:
  `Interpretable`,
  `ConsumeResult`,
  `CallableBody`,
  `SSACFGCallableBody`,
  with `#[interpret(crate = ...)]` targeting `::kirin_interpreter_2` by
  default.
- [ ] Implement the new derives against the v2 trait contracts and effect
  return types.
- [ ] Add focused tests proving the derives generate usable code for
  `kirin-interpreter-2`.
- [ ] Add focused toolkit tests covering explicit-only selector policy and
  `#[body]` validation.
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
2. `kirin-derive-toolkit` provides the shared forwarding/body helpers needed by
   the new derive package.
3. The derive surface is designed around the v2 runtime rather than around the
   old crate's abstractions.
4. The new derive package is test-covered and stable enough to be a
   prerequisite for downstream migration.
5. No downstream dialect crate has to mix old and new derive packages while the
   new derive API is still unsettled.
