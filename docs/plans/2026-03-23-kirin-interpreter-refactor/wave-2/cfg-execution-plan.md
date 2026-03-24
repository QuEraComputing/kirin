# Statement, Block, Region, and Call Execution

**Wave:** 2
**Agent role:** Implementer
**Estimated effort:** large

---

## Issue

This wave turns the runtime shell into a working concrete interpreter for
statement, block, and region execution. It must cover the first real execution
path that downstream dialects will rely on:

- `ExecStatement<'ir>` dispatch,
- `ExecBlock<'ir>` linear stepping and block-argument binding,
- `ExecRegion<'ir>` region scheduling,
- `CallableBody<'ir, I>` entry behavior for callable bodies,
- `ExecEffect::Call` with explicit frame-stack recursion,
- generic `ConsumeResult<'ir, I>` for nested execution boundaries.

The result should be a usable concrete CFG interpreter even before any graph
execution or broad dialect migration lands.

## Scope

**Files to add or modify:**

- `crates/kirin-interpreter-2/src/traits/*.rs`
- `crates/kirin-interpreter-2/src/stack/exec.rs`
- `crates/kirin-interpreter-2/src/stack/block.rs`
- `crates/kirin-interpreter-2/src/stack/region.rs`
- `crates/kirin-interpreter-2/src/stack/call.rs`
- `crates/kirin-interpreter-2/src/stack/callable.rs`
- `crates/kirin-interpreter-2/src/stack/transition.rs`
- `crates/kirin-interpreter-2/src/value_store.rs` or equivalent helper module
- `crates/kirin-interpreter-2/tests/stack_interp.rs`
- `crates/kirin-interpreter-2/tests/error_paths.rs`
- `crates/kirin-interpreter-2/tests/stage_dispatch.rs`

**Out of scope:**

- graph visitation behavior,
- derive macro support,
- broad downstream adoption in dialect crates.

## Key Behaviors To Land

- statement-driven `ExecEffect` dispatch,
- `Jump { block, args }` handling with block-arg binding,
- `Call { callee, stage, args }` with explicit pending-consumer metadata,
- `Return(V)` and `Yield(V)` resumption,
- dialect-owned outward arity handling through `ConsumeResult`,
- callable-body abstraction for standard CFG regions.

## Implementation Steps

- [ ] Implement `ExecStatement<'ir>` dispatch from the active cursor location.
- [ ] Implement `bind_block_args` and `ExecBlock<'ir>` stepping without assuming
  that all nested execution is block-shaped.
- [ ] Implement `ExecRegion<'ir>` scheduling for the current CFG region model,
  but keep region-owned state distinct from block-owned state.
- [ ] Implement pending nested-execution consumer storage on frames so calls and
  inline body execution resume through `ConsumeResult`.
- [ ] Implement `CallableBody<'ir, I>` and the blanket CFG-region path that
  replaces the old `SSACFGRegion` role.
- [ ] Add a set of small in-crate test dialects for:
  single-result return,
  multi-result consumption via dialect-owned unpacking,
  recursive calls,
  `Yield(V)` resumption,
  error cases at call and yield boundaries.
- [ ] Port the most relevant concrete-runtime tests from the old crate's
  `stack_interp.rs`, `stage_dispatch.rs`, and `error_paths.rs` into v2-specific
  equivalents rather than mechanically copying every old test.

## Validation

Run:

```bash
cargo nextest run -p kirin-interpreter-2 -E 'test(stack_interp|stage_dispatch|error_paths)'
cargo nextest run -p kirin-interpreter-2
```

## Success Criteria

1. `kirin-interpreter-2` can execute CFG-style programs with explicit call-stack
   recursion.
2. Result-convention policy lives in dialect `ConsumeResult` implementations,
   not in the framework.
3. The blanket callable-body path works for standard CFG regions.
4. The runtime is concrete-useful before any derive or downstream adoption work
   begins.
