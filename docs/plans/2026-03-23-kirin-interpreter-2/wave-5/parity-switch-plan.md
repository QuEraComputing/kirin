# Parity Matrix and Opt-In Replacement Plumbing

**Wave:** 5
**Agent role:** Implementer
**Estimated effort:** moderate

---

## Issue

The user goal is not just to create a new crate, but to replace the old one
once functionality matches. That requires a deliberate parity and rollout wave.

This wave should answer two questions explicitly:

1. what concrete functionality is now covered by `kirin-interpreter-2`, and
2. how can workspace users opt into it without breaking existing consumers.

The result should be a controlled opt-in switch, not a surprise replacement.

## Scope

**Files to add or modify:**

- `Cargo.toml`
- top-level `src/lib.rs` if re-exports or features need adjustment
- `crates/kirin-interpreter-2/README` or crate docs if needed
- `example/toy-lang` test or example entrypoints
- parity report or checklist under `docs/plans/2026-03-23-kirin-interpreter-2/`

**Out of scope:**

- deleting `kirin-interpreter`,
- removing old derive support,
- claiming abstract-interpretation parity.

## Required Deliverables

- a parity checklist covering:
  stage dispatch,
  breakpoints,
  runtime control surfaces,
  recursion,
  CFG execution,
  graph execution,
  pilot dialect coverage,
  derive support status
- an opt-in workspace path for using `kirin-interpreter-2`
- regression tests or example runs that compare old and new concrete runtimes on
  representative programs where both runtimes are expected to agree

## Implementation Steps

- [ ] Add an explicit feature or example path that lets users choose
  `kirin-interpreter-2` without changing current defaults.
- [ ] Build a small parity matrix against the old concrete interpreter for the
  features both runtimes intentionally share.
- [ ] Add dual-run tests in `example/toy-lang` or another suitable host harness
  where the same program is executed by both interpreters and their observable
  outputs are compared.
- [ ] Document known gaps that still block making v2 the default.
- [ ] Only after the parity matrix is satisfactory, propose the follow-up plan
  for default-switch and eventual deprecation of the old crate.

## Validation

Run:

```bash
cargo build --workspace
cargo nextest run --workspace
cargo test --doc --workspace
```

## Success Criteria

1. The workspace has a documented, test-backed opt-in path for the new runtime.
2. Parity claims are concrete rather than assumed.
3. The old crate remains available until the parity checklist says it is safe to
   switch consumers.
4. The next decision after this wave is narrow and explicit: either switch the
   default, or close specific remaining gaps first.
