# Kirin Interpreter Refactor Design

**Date:** 2026-03-23
**Primary crate:** `crates/kirin-interpreter-2`
**Pattern:** additive new crate + staged adoption

## Summary

This design set defines the execution model for `kirin-interpreter-2`, the new
derive package needed to target that runtime ergonomically, and the shared
derive-toolkit work required to keep those macros maintainable.

The concrete stack interpreter is the first implementation target. The runtime
and derive boundaries should still remain suitable for a future abstract
interpreter, but abstract interpretation is explicitly out of scope for this
first crate.

## Goals

- make execution shape-aware rather than block-centric
- keep recursion and nested execution on an explicit interpreter-managed frame
  stack
- separate semantic effects from debugger/runtime stop reasons
- keep outward result conventions dialect-owned rather than imposed by the core
  runtime
- reuse raw `kirin_ir::Product<V>` where the public protocol needs structural
  value lists
- finish the new derive package and shared toolkit support before downstream
  dialect migration begins

## Non-Goals

- retrofitting `kirin-interpreter` in place
- retrofitting `kirin-derive-interpreter` in place
- defining a public fully generic machine abstraction
- defining a core framework trait for implicit tuple or product packing
- implementing abstract interpretation in the first `kirin-interpreter-2`
  rollout

## Document Map

- [runtime-model.md](runtime-model.md)
  Core trait family, stage dispatch, typed-stage facade, cursors, runtime loop,
  control surfaces, and frame storage.
- [result-conventions.md](result-conventions.md)
  `Product<V>` usage, `Return`/`Yield`, `ConsumeResult`, explicit call-stack
  handling, and dialect-owned result adaptation.
- [derive-and-tooling.md](derive-and-tooling.md)
  `kirin-derive-interpreter-2`, macro contracts, helper attributes,
  `kirin-derive-toolkit` templates, and darling/layout interaction.
- [testing-and-rollout.md](testing-and-rollout.md)
  Test strategy, graph-validation requirements, initial implementation scope,
  and follow-up planning.

## Key Decisions Snapshot

- Public interpreter APIs are a trait family, not one block-centric supertrait.
- `ExecutionCursor` stays internal in v1 and is a closed enum by execution
  shape.
- `ExecutionLocation` stays statement-based in v1 for uniform breakpoint
  behavior across blocks and graphs.
- `ExecEffect<V>` keeps `Call` so recursion stays on the interpreter frame
  stack.
- `Return(V)` and `Yield(V)` stay single-valued; outward arity adaptation is
  dialect-owned through `ConsumeResult`.
- `kirin-derive-interpreter-2` is a separate crate and depends on shared
  forwarding/body helpers in `kirin-derive-toolkit`.
