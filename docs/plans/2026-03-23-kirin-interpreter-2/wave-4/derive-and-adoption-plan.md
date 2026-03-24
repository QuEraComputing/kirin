# Derive Support and Pilot Dialect Adoption

**Wave:** 4
**Agent role:** Implementer
**Estimated effort:** large

---

## Issue

Once the v2 runtime is real, downstream crates need a practical way to target
it. This wave covers two connected tasks:

1. update `kirin-derive-interpreter` so it can generate code for the new trait
   family and effect protocol, and
2. prove the API on real dialect crates by migrating a pilot set from
   `kirin-interpreter` to `kirin-interpreter-2`.

The pilot set should be:

- `kirin-function`
- `kirin-scf`
- `kirin-cf`
- `example/toy-lang`

`kirin-cf` is included because region execution and structured control flow are
hard to validate in practice without real branch semantics.

## Scope

**Files to add or modify:**

- `docs/plans/2026-03-23-kirin-interpreter-2/migration-guide.md`
- `crates/kirin-derive-interpreter/src/*`
- `crates/kirin-interpreter-2/Cargo.toml`
- `crates/kirin-interpreter-2/src/lib.rs`
- `crates/kirin-function/src/*`
- `crates/kirin-scf/src/*`
- `crates/kirin-cf/src/*`
- `example/toy-lang/src/*`
- new or updated tests in the pilot crates

**Out of scope:**

- removing `kirin-interpreter`,
- making `kirin-interpreter-2` the workspace default,
- broad migration of every dialect crate.

## Adoption Strategy

Do not keep the old and new interpreter integrations side by side inside a
migrated pilot crate. That creates avoidable trait-name overlap and import
ambiguity.

Migration precondition:

- `kirin-interpreter-2` must already have several runtime tests in place,
  including CFG execution, call/result-consumer coverage, and the graph-facing
  tests from Waves 2 and 3.

Per-crate migration sequence:

1. remove the crate's dependency on `kirin-interpreter`,
2. remove old interpreter imports, derive wiring, and impl modules,
3. add the `kirin-interpreter-2` dependency,
4. switch the crate to the new interpreter API,
5. run that crate's tests before moving to the next crate.

This keeps trait-name overlap out of migrated dialect crates and makes each
adoption step reviewable as a self-contained switch.

## Derive Work

Treat derive support as a real API migration:

- `Interpretable` derive must emit the new `ExecEffect` return type.
- The old `CallSemantics` / `SSACFGRegion` derives need either new v2-focused
  derives or compatibility expansions that can target `CallableBody` and the
  new execution model.
- Crate-path override support must work for `::kirin_interpreter_2`, not just
  `::kirin_interpreter`.

Do not assume this is a path-only rewrite. The generated bounds and return types
must change materially.

## Implementation Steps

- [ ] Extend `kirin-derive-interpreter` to support the new crate path and v2
  trait contracts.
- [ ] Decide whether the new derive names should be `CallableBody` /
  `SSACFGRegion` / compatibility wrappers, and keep that choice consistent
  across docs and codegen.
- [ ] Write `docs/plans/2026-03-23-kirin-interpreter-2/migration-guide.md`
  documenting the per-crate migration sequence for downstream dialect crates.
- [ ] Confirm that `kirin-interpreter-2` test coverage is already strong enough
  to support downstream migration before switching any pilot crate.
- [ ] Migrate `kirin-function`, `kirin-scf`, and `kirin-cf` one crate at a
  time:
  remove the old interpreter dependency,
  remove old interpreter wiring,
  add the new interpreter dependency,
  switch to the v2 API,
  run tests.
- [ ] Wire `example/toy-lang` to run with the new interpreter as an explicit
  path or feature after its supporting dialect crates have switched.
- [ ] Add integration tests that exercise real function calls, structured
  control flow, and branch behavior through `kirin-interpreter-2`.

## Validation

Run:

```bash
cargo build -p kirin-derive-interpreter
cargo nextest run -p kirin-function -p kirin-scf -p kirin-cf -p toy-lang
cargo nextest run -p kirin-interpreter-2
```

## Success Criteria

1. Real dialect crates can migrate to `kirin-interpreter-2` without carrying
   both interpreter APIs at once.
2. The derive crate can generate correct v2 code rather than hardcoding old
   continuation-era assumptions.
3. `example/toy-lang` can exercise the new runtime on nontrivial programs.
4. The workspace has a written migration guide and a credible pilot migration
   path before any default switch is considered.
