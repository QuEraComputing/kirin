# Bootstrap the New Crate and Public API Skeleton

**Wave:** 0
**Agent role:** Implementer
**Estimated effort:** moderate

---

## Issue

`kirin-interpreter-2` needs to exist as a first-class workspace crate before any
runtime work can land. The bootstrap should establish:

- workspace membership and dependency plumbing,
- the public module layout for the new trait family,
- the basic error/effect/event/location types that downstream code will import,
- a lean `mod.rs` structure that matches the repo's module conventions, and
- a minimal compile-time smoke test surface.

This wave should not yet attempt to execute programs. Its job is to give the
next waves a stable crate boundary and a clean namespace.

## Scope

**Files to add or modify:**

- `Cargo.toml`
- `crates/kirin-interpreter-2/Cargo.toml`
- `crates/kirin-interpreter-2/src/lib.rs`
- `crates/kirin-interpreter-2/src/prelude.rs`
- `crates/kirin-interpreter-2/src/traits/mod.rs`
- `crates/kirin-interpreter-2/src/traits/*.rs`
- `crates/kirin-interpreter-2/src/runtime/mod.rs`
- `crates/kirin-interpreter-2/src/runtime/effect.rs`
- `crates/kirin-interpreter-2/src/runtime/location.rs`
- `crates/kirin-interpreter-2/src/runtime/status.rs`
- `crates/kirin-interpreter-2/src/error.rs`
- `crates/kirin-interpreter-2/tests/bootstrap.rs`

**Out of scope:**

- concrete execution behavior,
- frame stack implementation,
- block/region/graph stepping,
- derive macro integration,
- downstream dialect changes.

## Recommended Module Layout

Use three top-level groups:

- `traits/`
  Public trait family only.
- `runtime/`
  Public execution protocol types (`ExecEffect`, `RunStatus`,
  `ExecutionLocation`).
- `stack/`
  Reserved for the concrete stack interpreter in later waves.

`lib.rs` should stay lean and only declare modules, re-export the public surface,
and define the crate prelude.

## Implementation Steps

- [ ] Add `crates/kirin-interpreter-2` to workspace members and
  `workspace.dependencies`.
- [ ] Create the new crate with no `derive` feature yet. Keep feature surface
  minimal until the trait/API shape stabilizes.
- [ ] Define the public trait skeletons described by the design doc:
  `ValueStore`, `StageAccess<'ir>`, `ExecStatement<'ir>`, `ExecBlock<'ir>`,
  `ExecRegion<'ir>`, `VisitDiGraph<'ir>`, `VisitUnGraph<'ir>`,
  `CallExecutor<'ir>`, `CallableBody<'ir, I>`, `DebugDriver<'ir>`,
  `ConsumeResult<'ir, I>`.
- [ ] Define the public runtime protocol types:
  `ExecEffect<V>`, `RunStatus<V>`, `StopReason`, `ExecutionLocation`.
- [ ] Define `InterpreterError` variants needed by the design but avoid baking in
  implementation-only errors that belong to later waves.
- [ ] Add crate-level documentation summarizing the separation between semantic
  effects and runtime stop events.
- [ ] Add a small bootstrap test that proves the crate compiles, exports the
  intended names, and can be imported from an integration test.

## Validation

Run:

```bash
cargo build -p kirin-interpreter-2
cargo nextest run -p kirin-interpreter-2
```

## Success Criteria

1. The new crate is part of the workspace and builds independently.
2. The public trait and protocol surface matches the approved design doc.
3. No downstream crate has been forced to migrate yet.
4. The module structure is ready for Waves 1-5 without stuffing all logic into
   `lib.rs`.
