# Kirin Interpreter 2 — Implementation Plan Index

**Date:** 2026-03-23
**Design document:** `docs/design/interpreter-2-execution-model.md`
**Pattern:** additive new crate + staged adoption
**Primary crate:** `crates/kirin-interpreter-2`

---

## Summary

This plan builds `kirin-interpreter-2` as a new concrete interpreter crate
without destabilizing the existing `kirin-interpreter` crate. The work is
sequenced so that:

1. the new crate can compile and host its own tests early,
2. the concrete runtime lands before dialect adoption,
3. graph execution is validated independently from CFG execution, and
4. a new v2-specific derive package is finished before any downstream dialect
   migration begins,
5. downstream migration remains opt-in until parity is demonstrated, and
6. each migrated dialect crate switches in one direction from
   `kirin-interpreter` to `kirin-interpreter-2` instead of carrying both
   interpreter integrations at once.

The plan intentionally separates "new runtime exists" from "workspace has
switched to it". The old crate remains authoritative until the parity wave.

## Dependency Graph

```text
wave-0 (bootstrap + crate skeleton)
   |
wave-1 (runtime kernel + staged facade + dispatch/frame stack)
   |
wave-2 (statement/block/region execution + calls + result consumers)
   |
wave-3 (graph visitation + DiGraph toy-language tests)
   |
wave-4 (new derive package for interpreter-2)
   |
wave-5 (migration guide + pilot dialect migration)
   |
wave-6 (parity checks + opt-in switch plumbing)
```

## Waves

### Wave 0

**Depends on:** nothing

| Plan File | Title | Agent Role | Crate(s) |
|-----------|-------|------------|----------|
| `wave-0/bootstrap-plan.md` | Bootstrap the new crate and public API skeleton | Implementer | workspace, kirin-interpreter-2 |

### Wave 1

**Depends on:** Wave 0 complete

| Plan File | Title | Agent Role | Crate(s) |
|-----------|-------|------------|----------|
| `wave-1/runtime-kernel-plan.md` | Concrete runtime kernel and typed-stage facade | Implementer | kirin-interpreter-2 |

### Wave 2

**Depends on:** Wave 1 complete

| Plan File | Title | Agent Role | Crate(s) |
|-----------|-------|------------|----------|
| `wave-2/cfg-execution-plan.md` | Statement, block, region, and call execution | Implementer | kirin-interpreter-2 |

### Wave 3

**Depends on:** Wave 2 complete

| Plan File | Title | Agent Role | Crate(s) |
|-----------|-------|------------|----------|
| `wave-3/graph-visitation-plan.md` | Graph visitation and computational-graph validation | Implementer | kirin-interpreter-2, kirin-test-languages or kirin-test-utils (if shared fixtures are needed) |

### Wave 4

**Depends on:** Waves 2 and 3 complete

| Plan File | Title | Agent Role | Crate(s) |
|-----------|-------|------------|----------|
| `wave-4/derive-package-plan.md` | New derive package for interpreter-2 | Implementer | workspace, kirin-derive-interpreter-2, kirin-interpreter-2 |

### Wave 5

**Depends on:** Wave 4 complete

| Plan File | Title | Agent Role | Crate(s) |
|-----------|-------|------------|----------|
| `wave-5/migration-and-adoption-plan.md` | Migration guide and pilot dialect migration | Implementer | kirin-derive-interpreter-2, kirin-interpreter-2, kirin-function, kirin-scf, kirin-cf, example/toy-lang |

### Wave 6

**Depends on:** Wave 5 complete

| Plan File | Title | Agent Role | Crate(s) |
|-----------|-------|------------|----------|
| `wave-6/parity-switch-plan.md` | Parity matrix and opt-in replacement plumbing | Implementer | workspace, kirin, kirin-interpreter-2, example/toy-lang |

## Verification Checkpoints

After each wave:

1. `cargo build -p kirin-interpreter-2`
2. `cargo nextest run -p kirin-interpreter-2`

Additional checkpoints by wave:

- Wave 0: `cargo build -p kirin-derive-interpreter` to ensure bootstrap changes do not break derive compilation paths.
- Wave 1: targeted stage and control-surface tests in `kirin-interpreter-2`.
- Wave 2: targeted call, recursion, breakpoint, and result-consumer tests.
- Wave 3: targeted graph execution tests, including the DiGraph output-equivalence case.
- Wave 4: `cargo build -p kirin-derive-interpreter-2` and derive-crate tests.
- Wave 5: `cargo nextest run -p kirin-function -p kirin-scf -p kirin-cf -p toy-lang`
- Wave 6: `cargo build --workspace`, `cargo nextest run --workspace`, `cargo test --doc --workspace`

## Major Risks

### Risk 1: Trait-name overlap during adoption

`kirin-interpreter` and `kirin-interpreter-2` intentionally reuse names such as
`Interpretable` and `StageAccess`.

Mitigation: once `kirin-interpreter-2` has enough crate-local tests to act as a
stable target, migrate each downstream dialect crate in one direction:

1. remove its dependency on `kirin-interpreter`,
2. remove old-interpreter imports and impl wiring,
3. add the `kirin-interpreter-2` dependency, and
4. switch the crate to the new interpreter API.

Wave 5 includes a migration-guide step so this sequence is written down and
reused consistently.

### Risk 2: Derive codegen assumes old continuation/call abstractions

`kirin-derive-interpreter` currently targets the old crate's `Continuation`,
`CallSemantics`, and `SSACFGRegion` APIs.

Mitigation: do not retrofit the old derive crate in place as the migration
prerequisite. Build a separate `kirin-derive-interpreter-2` package whose macro
surface is designed around the new runtime traits and effect protocol. Finish
that crate before downstream migration starts.

### Risk 3: Graph API can drift before a concrete test exists

The graph-visitation surface is deliberately abstract. The DiGraph toy-language
test in Wave 3 is the first concrete guardrail that ensures the API is capable
of producing useful runtime behavior without forcing one universal graph
scheduler.

## Recommended Merge Strategy

- Merge Wave 0 and Wave 1 before any downstream crate sees the new runtime.
- Keep Wave 2 self-contained inside `kirin-interpreter-2`; avoid touching
  dialect crates until call/result mechanics are proven.
- Use Wave 3 to stabilize graph APIs before designing the derive package API.
- Treat Wave 4 as a hard prerequisite: the new derive package must be finished
  before any pilot dialect migration starts.
- Keep Wave 5 opt-in, but migrate each pilot dialect crate in one direction once
  both `kirin-interpreter-2` and `kirin-derive-interpreter-2` are ready.
- Treat Wave 6 as the release gate for "ready to switch consumers", not for
  deleting the old runtime.
