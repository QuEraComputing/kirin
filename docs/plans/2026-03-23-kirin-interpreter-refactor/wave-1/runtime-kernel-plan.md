# Concrete Runtime Kernel and Typed-Stage Facade

**Wave:** 1
**Agent role:** Implementer
**Estimated effort:** large

---

## Issue

The new crate needs a concrete runtime kernel before any statement or body
semantics can execute. This wave establishes the runtime-owned parts of the
design:

- `StackInterpreter<'ir, V, S, E, G>` (or equivalent naming),
- reusable `Frame` / `FrameStack` storage,
- internal `ExecutionCursor` and per-shape cursor structs,
- stage-dynamic dispatch cache and per-frame dispatch entries,
- typed `Staged<'a, 'ir, I, L>` host façade,
- interpreter-global state access,
- runtime control surfaces such as fuel, depth, breakpoints, and halt status.

This wave is intentionally about runtime mechanics, not about the semantics of
blocks, regions, or graphs yet.

## Scope

**Files to add or modify:**

- `crates/kirin-interpreter-2/src/stack/mod.rs`
- `crates/kirin-interpreter-2/src/stack/interp.rs`
- `crates/kirin-interpreter-2/src/stack/frame.rs`
- `crates/kirin-interpreter-2/src/stack/frame_stack.rs`
- `crates/kirin-interpreter-2/src/stack/cursor.rs`
- `crates/kirin-interpreter-2/src/stack/dispatch.rs`
- `crates/kirin-interpreter-2/src/stack/staged.rs`
- `crates/kirin-interpreter-2/src/stack/debug.rs`
- `crates/kirin-interpreter-2/src/stack/control.rs`
- `crates/kirin-interpreter-2/src/lib.rs`
- `crates/kirin-interpreter-2/tests/stage_dispatch.rs`
- `crates/kirin-interpreter-2/tests/control_surfaces.rs`

**Out of scope:**

- statement dispatch into dialect semantics,
- block/region stepping,
- call entry and return handling,
- graph visitation behavior.

## Design Constraints

- Keep `ExecutionCursor` internal and closed by shape.
- Preserve a reusable frame-stack kernel so a future abstract interpreter can
  reuse the storage model even if it does not reuse the concrete runtime.
- Keep debugger stop reasons separate from `ExecEffect<V>`.
- Retain ergonomic stage-typed entrypoints through `Staged<'a, ...>`.

## Implementation Steps

- [ ] Implement `Frame<V, X>` and `FrameStack<V, X>` in the new crate rather
  than reaching into the old crate.
- [ ] Define internal cursor structs for `BlockCursor`, `RegionCursor`,
  `DiGraphCursor`, and `UnGraphCursor`, plus location projection helpers.
- [ ] Implement the concrete interpreter struct with:
  value storage,
  frame stack,
  dispatch cache,
  global state `G`,
  breakpoint set,
  fuel/max-depth configuration.
- [ ] Implement `StageAccess<'ir>` and the typed `Staged<'a, ...>` handle.
- [ ] Port the stage-dynamic dispatch pattern from the old interpreter into a
  v2-appropriate cache keyed by `CompileStage`.
- [ ] Implement driver-level stop handling for:
  breakpoints,
  fuel exhaustion,
  explicit halt,
  stack depth overflow.
- [ ] Add targeted tests for:
  stage resolution,
  dispatch-cache use across stages,
  breakpoint stop and resume at `ExecutionLocation`,
  fuel and depth limits,
  global-state access.

## Validation

Run:

```bash
cargo build -p kirin-interpreter-2
cargo nextest run -p kirin-interpreter-2 -E 'test(stage_dispatch|control_surfaces)'
```

Then run the full crate test suite:

```bash
cargo nextest run -p kirin-interpreter-2
```

## Success Criteria

1. The new crate has a functioning concrete runtime shell with explicit stack
   frames and typed-stage entrypoints.
2. Runtime control surfaces exist and are test-covered.
3. Stage-polymorphic execution no longer depends on repeated lookups or Rust
   call frames.
4. Later waves can plug in statement and body execution without redesigning the
   runtime kernel.
