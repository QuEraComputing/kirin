+++
rfc = "0012"
title = "stage-dynamic interpreter dispatch and call-side resolution"
status = "Draft"
agents = ["codex"]
authors = ["Roger-luo <code@rogerluo.dev>"]
created = "2026-02-27T03:34:09.8553Z"
last_updated = "2026-02-27T03:34:09.8553Z"
+++

# RFC 0012: stage-dynamic interpreter dispatch and call-side resolution

## Summary

This RFC refactors interpreter execution to support cross-stage runtime dispatch
while preserving strict typed execution APIs. Function resolution from
abstract-function identity to concrete specialization is owned by call
statement semantics (`Interpretable`), not by the interpreter core. To encode
resolved call intent across stage boundaries, `Continuation::Call` gains a
`CompileStage` field in addition to `SpecializedFunction`. Frame state in both
concrete and abstract interpreters becomes stage-aware so recursive mixed-stage
call chains (for example `A -> C -> B -> A`) are modeled correctly.

## Motivation

- Problem: current execution entrypoints in
  `crates/kirin-interpreter/src/stack.rs` and
  `crates/kirin-interpreter/src/abstract_interp/fixpoint.rs` assume a
  compile-time fixed `L` through `active_stage_info::<L>()`. This does not
  match workloads where call chains move across compile stages.
- Problem: call target resolution policy is semantic behavior of call-like
  operations, but the framework direction requires interpreter core to stay a
  generic executor.
- Why now: we need a single, explicit contract before implementing
  multi-stage/cross-stage call behavior and recursion in interpreter execution.
- Stakeholders:
  - `kirin-interpreter` maintainers and users of concrete + abstract execution
  - dialect maintainers implementing call semantics (`Interpretable`)
  - downstream analysis users relying on summary caching behavior

## Goals

- Support dynamic execution driven by frame stage, using existing pipeline stage
  dispatch infrastructure in `kirin-ir`.
- Keep typed `::<L>` execution APIs and make them strict on stage mismatch.
- Make call semantics responsible for specialization choice, with initial
  policy `unique-or-error`.
- Ensure recursive cross-stage call chains preserve correct stage context per
  frame.
- Remove stage-assumption panics from execution paths and replace with explicit
  errors.

## Non-goals

- Introduce signature-semantics based specialization dispatch in this RFC.
- Redesign function dialect syntax or parser grammar.
- Remove typed execution APIs.
- Add new IR-level stage-dispatch primitives in `kirin-ir` (existing APIs are
  sufficient).

## Guide-level Explanation

A call-like dialect operation resolves abstract call identity and returns a
resolved continuation:

```rust
Continuation::Call {
    callee: SpecializedFunction,
    stage: CompileStage,
    args,
    result,
}
```

The interpreter consumes this continuation and performs execution mechanics
only:

- advance caller cursor
- push callee frame
- switch execution context to the callee stage from continuation payload
- bind entry block arguments
- continue stepping

Typed APIs remain valid when users know the active language/stage pair:

- `step::<L>`
- `advance::<L>`
- `run::<L>`
- `run_until_break::<L>`

These typed APIs become strict: if the current frame stage does not contain
`StageInfo<L>`, they return an explicit error. Dynamic APIs are introduced for
mixed-stage flows:

- `step_dyn`
- `advance_dyn`
- `run_dyn`
- `run_until_break_dyn`
- `call_dyn`
- `analyze_dyn`

## Reference-level Explanation

### API and syntax changes

Interpreter-side public API changes in `kirin-interpreter`:

- `Continuation::Call` gains `stage: CompileStage` in
  `crates/kirin-interpreter/src/control.rs`.
- `Frame` gains stage storage in `crates/kirin-interpreter/src/frame.rs`.
- `StackInterpreter` gains dynamic execution entrypoints in
  `crates/kirin-interpreter/src/stack.rs`.
- `AbstractInterpreter` gains dynamic analysis entrypoint in
  `crates/kirin-interpreter/src/abstract_interp/fixpoint.rs`.
- Typed entrypoints remain, with strict stage mismatch errors.

Error surface expansion in `crates/kirin-interpreter/src/error.rs`:

- typed stage mismatch
- missing function-stage mapping
- no specialization at target stage
- ambiguous specialization at target stage

Abstract interpreter cache behavior:

- summary keys include compile stage together with callee identity to prevent
  cross-stage collisions.

### Semantics and invariants

Call ownership boundary:

- resolution from `(Function, CompileStage)` to concrete `SpecializedFunction`
  is done in call-op `Interpretable` implementation.
- interpreter core does not choose between specializations; it executes resolved
  call intent.

`Continuation::Call` invariant:

- `callee` must be valid in `stage`.

Frame invariant:

- every frame carries exactly one `CompileStage`, and all statement decoding for
  that frame must use that stage.

Typed execution invariant:

- `::<L>` methods require `StageInfo<L>` in current frame stage; otherwise
  return explicit error.

Resolution invariant (`unique-or-error`, initial policy):

- zero live specialization: error
- more than one live specialization: error
- exactly one live specialization: success

### Crate impact matrix

| crate | impact | tests to update |
| --- | --- | --- |
| `kirin-interpreter` | dynamic execution path, stage-aware frames, continuation payload change, stage-aware abstract summary keys | `tests/concrete_interp.rs`, `tests/abstract_fixpoint.rs`, `tests/test_dialect_coverage.rs`, new mixed-stage recursion tests |
| `kirin-function` or call-owning dialect crate | call `Interpretable` resolves `(Function, CompileStage)` and emits `Continuation::Call { callee, stage, ... }` | call-resolution behavior tests for missing/none/ambiguous/unique cases |
| `kirin-ir` | no API changes expected; existing stage dispatch APIs are consumed from interpreter/call semantics | existing `stage_dispatch` tests unchanged |

## Drawbacks

- Execution model is more complex due to dual typed and dynamic API paths.
- Stage-aware frame and summary bookkeeping increases implementation surface.
- Existing code matching/constructing `Continuation::Call` must be updated.

## Rationale and Alternatives

### Chosen approach rationale

- Keeps semantic policy where it belongs: call semantics decide call target
  resolution.
- Keeps interpreter reusable and generic: execute resolved control flow, do not
  encode dialect-specific call resolution rules.
- Uses a minimal payload change (`stage` field) instead of introducing a new
  wrapper type.

### Alternative A

- Description: interpreter resolves abstract function calls internally.
- Pros: fewer responsibilities for dialect call ops.
- Cons: couples interpreter core to call-resolution policy and function
  semantics; harder to keep executor generic.
- Reason not chosen: violates desired ownership boundary.

### Alternative B

- Description: introduce a dedicated `ResolvedCallee` wrapper type in
  continuation.
- Pros: explicit type-level marker for resolved call targets.
- Cons: extra type/API churn without additional capability over adding
  `stage` to existing `Continuation::Call`.
- Reason not chosen: explicit decision to keep call payload shape minimal.

### Alternative C

- Description: remove typed APIs and use dynamic dispatch only.
- Pros: single execution path.
- Cons: loses strict/typed ergonomics for known-language execution.
- Reason not chosen: typed APIs remain valuable and intentionally supported.

## Prior Art

- Existing stage dispatch pattern in
  `crates/kirin-ir/src/stage_dispatch.rs` (action-based runtime dialect
  dispatch).
- Existing interpreter split between concrete and abstract engines where
  execution mechanics are shared but walking strategies differ.

## Backward Compatibility and Migration

- Breaking changes:
  - `Continuation::Call` constructors and pattern matches must include `stage`.
  - frame construction paths must provide frame stage.
- Migration steps:
  1. Update all `Continuation::Call` construction sites to include
     `stage: CompileStage`.
  2. Update all `Continuation::Call` pattern matches to destructure `stage`.
  3. Move abstract-function-to-specialized-function resolution logic into
     call-op `Interpretable` implementations.
  4. Route mixed-stage execution flows to dynamic APIs (`*_dyn`).
- Compatibility strategy:
  - typed APIs remain available and are strict by design.
  - dynamic APIs are additive and used for cross-stage chains.

## How to Teach This

- Teach “semantic resolution vs execution” boundary:
  - call semantics resolve targets;
  - interpreters execute resolved targets.
- Add a small example in interpreter docs showing:
  - typed API mismatch behavior,
  - dynamic API handling mixed-stage recursive calls.
- Update relevant crate docs/comments near `Continuation::Call`, frame, and
  call `Interpretable` implementations.

## Reference Implementation Plan

1. Update continuation/frame models (`control.rs`, `frame.rs`) for stage-carrying calls and
   per-frame stage.
2. Add strict typed-stage checks and dynamic execution APIs in `stack.rs`.
3. Make abstract interpreter stage-aware (`interp.rs`, `fixpoint.rs`), including summary keys.
4. Refactor call-driving helpers in `eval/call.rs` and `eval/block.rs` to use dynamic nested-call
   execution.
5. Update call semantics in call-owning dialect crate(s) to resolve `(Function, CompileStage)` and
   emit `Continuation::Call { callee, stage, ... }`.
6. Add mixed-stage recursion tests and explicit call-resolution failure-mode tests.

### Acceptance Criteria

- [ ] Cross-stage recursive call chain executes with correct per-frame stage fidelity.
- [ ] Typed APIs return explicit stage mismatch error when `L` is not present in frame stage.
- [ ] Dynamic APIs execute mixed-stage call chains correctly.
- [ ] Call-resolution failures are surfaced from call semantics: missing mapping, no
      specialization, ambiguous specialization.
- [ ] Abstract summary caches do not collide across stages.
- [ ] `cargo test -p kirin-interpreter` passes with updated and new tests.

### Tracking Plan

- Tracking issue: TBD
- Implementation PRs: TBD
- Follow-up tasks:
  - optional signature-semantics dispatch policy after `unique-or-error`

## Unresolved Questions

- Should shared helper APIs for call resolution live in `kirin-interpreter`, or
  stay dialect-local initially and be unified later?
- When should signature-based specialization selection replace or augment
  `unique-or-error`?

## Future Possibilities

- Extend call resolution policy to leverage `SignatureSemantics` once call-site
  signature context is standardized.
- Add optional diagnostics tooling for stage-transition traces during
  mixed-stage interpretation.

## Revision Log

| Date | Change |
| --- | --- |
| 2026-02-27T03:34:09.8553Z | RFC created from template |
| 2026-02-27 | Filled RFC 0012 with stage-dynamic interpreter and call-side resolution design |
