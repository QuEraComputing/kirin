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
4. downstream migration remains opt-in until parity is demonstrated.

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
wave-4 (derive support + pilot dialect adoption)
   |
wave-5 (parity checks + opt-in switch plumbing)
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

**Depends on:** Wave 2 complete for runtime integration. Wave 3 preferred before merge so graph-related API drift is caught before downstream adoption.

| Plan File | Title | Agent Role | Crate(s) |
|-----------|-------|------------|----------|
| `wave-4/derive-and-adoption-plan.md` | Derive support and pilot dialect adoption | Implementer | kirin-derive-interpreter, kirin-interpreter-2, kirin-function, kirin-scf, kirin-cf, example/toy-lang |

### Wave 5

**Depends on:** Waves 3 and 4 complete

| Plan File | Title | Agent Role | Crate(s) |
|-----------|-------|------------|----------|
| `wave-5/parity-switch-plan.md` | Parity matrix and opt-in replacement plumbing | Implementer | workspace, kirin, kirin-interpreter-2, example/toy-lang |

## Verification Checkpoints

After each wave:

1. `cargo build -p kirin-interpreter-2`
2. `cargo nextest run -p kirin-interpreter-2`

Additional checkpoints by wave:

- Wave 0: `cargo build -p kirin-derive-interpreter` to ensure bootstrap changes do not break derive compilation paths.
- Wave 1: targeted stage and control-surface tests in `kirin-interpreter-2`.
- Wave 2: targeted call, recursion, breakpoint, and result-consumer tests.
- Wave 3: targeted graph execution tests, including the DiGraph output-equivalence case.
- Wave 4: `cargo nextest run -p kirin-function -p kirin-scf -p kirin-cf -p toy-lang`
- Wave 5: `cargo build --workspace`, `cargo nextest run --workspace`, `cargo test --doc --workspace`

## Major Risks

### Risk 1: Trait-name overlap during adoption

`kirin-interpreter` and `kirin-interpreter-2` intentionally reuse names such as
`Interpretable` and `StageAccess`. Downstream crates will need explicit import
discipline or separate `interpret_v2` modules during the dual-runtime period.

### Risk 2: Derive codegen assumes old continuation/call abstractions

`kirin-derive-interpreter` currently targets the old crate's `Continuation`,
`CallSemantics`, and `SSACFGRegion` APIs. Wave 4 must treat derive support as a
real codegen change, not a path-only rename.

### Risk 3: Graph API can drift before a concrete test exists

The graph-visitation surface is deliberately abstract. The DiGraph toy-language
test in Wave 3 is the first concrete guardrail that ensures the API is capable
of producing useful runtime behavior without forcing one universal graph
scheduler.

## Recommended Merge Strategy

- Merge Wave 0 and Wave 1 before any downstream crate sees the new runtime.
- Keep Wave 2 self-contained inside `kirin-interpreter-2`; avoid touching
  dialect crates until call/result mechanics are proven.
- Use Wave 3 to stabilize graph APIs before broad derive or dialect adoption.
- Keep Wave 4 opt-in. Do not remove or rewrite the old interpreter crate yet.
- Treat Wave 5 as the release gate for "ready to switch consumers", not for
  deleting the old runtime.
