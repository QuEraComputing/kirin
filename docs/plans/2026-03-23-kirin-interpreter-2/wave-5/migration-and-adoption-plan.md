# Migration Guide And Pilot Dialect Migration

**Wave:** 5
**Agent role:** Implementer
**Estimated effort:** large

---

## Issue

Once both `kirin-interpreter-2` and `kirin-derive-interpreter-2` are in place,
the next step is downstream migration. That migration should be disciplined and
one-way per crate, not a long-lived dual-runtime coexistence inside the same
dialect crate.

This wave covers two tasks:

1. write the migration guide that documents the required switch sequence, and
2. migrate a pilot set of dialect crates and host code using that sequence.

The pilot set should be:

- `kirin-function`
- `kirin-scf`
- `kirin-cf`
- `example/toy-lang`

## Scope

**Files to add or modify:**

- `docs/plans/2026-03-23-kirin-interpreter-2/migration-guide.md`
- `crates/kirin-function/src/*`
- `crates/kirin-scf/src/*`
- `crates/kirin-cf/src/*`
- `example/toy-lang/src/*`
- crate manifests for the pilot crates as needed
- new or updated tests in the pilot crates

**Out of scope:**

- deleting `kirin-interpreter`,
- deleting `kirin-derive-interpreter`,
- making v2 the workspace default,
- broad migration of every dialect crate.

## Migration Preconditions

Before migrating any downstream dialect crate:

- `kirin-interpreter-2` must already have strong crate-local runtime tests,
  including CFG execution, call/result-consumer coverage, and graph-facing
  tests,
- `kirin-derive-interpreter-2` must already be implemented and tested,
- the v2 derive API must be stable enough that downstream crates are not
  expected to churn immediately afterward.

## Required Migration Sequence

For each migrated dialect crate:

1. remove the dependency on `kirin-interpreter`,
2. remove old interpreter imports, derive usage, and impl wiring,
3. remove the dependency on `kirin-derive-interpreter` if it is no longer used,
4. add the dependencies on `kirin-interpreter-2` and
   `kirin-derive-interpreter-2`,
5. switch the crate to the new interpreter and derive APIs,
6. run that crate's tests before moving to the next crate.

This is the migration sequence that addresses the trait-name overlap risk.

## Implementation Steps

- [ ] Write `docs/plans/2026-03-23-kirin-interpreter-2/migration-guide.md`
  documenting the migration sequence and required preconditions.
- [ ] Migrate `kirin-function` using the one-way switch process and run its
  tests.
- [ ] Migrate `kirin-cf` and `kirin-scf` using the same process and run their
  tests.
- [ ] Migrate `example/toy-lang` after its supporting dialect crates have
  switched.
- [ ] Add or update integration tests that exercise real function calls,
  structured control flow, and branch behavior through the new runtime.
- [ ] Record any migration friction that should feed back into the guide before
  moving to the parity wave.

## Validation

Run:

```bash
cargo nextest run -p kirin-function -p kirin-scf -p kirin-cf -p toy-lang
cargo nextest run -p kirin-interpreter-2
cargo nextest run -p kirin-derive-interpreter-2
```

## Success Criteria

1. The migration guide exists and matches the successful pilot migration
   sequence.
2. Real dialect crates can migrate to `kirin-interpreter-2` and
   `kirin-derive-interpreter-2` without carrying old and new interpreter APIs at
   once.
3. `example/toy-lang` can exercise the new runtime on nontrivial programs after
   the pilot dialects switch.
4. The parity wave can now compare a real migrated slice of the workspace
   instead of only in-crate tests.
