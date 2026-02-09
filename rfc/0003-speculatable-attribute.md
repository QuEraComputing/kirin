+++
rfc = "0003"
title = "Speculatable Attribute"
status = "Draft"
agents = ["codex"]
authors = ["Roger-luo <code@rogerluo.dev>"]
created = "2026-02-09T04:05:27.219042Z"
last_updated = "2026-02-09T04:05:27.219042Z"
dependencies = ["0002"]
+++

# RFC 0003: Speculatable Attribute

## Summary

Add a new dialect property attribute `#[kirin(speculatable)]` to mark
operations that are safe to execute speculatively when their result is unused.
This RFC introduces a dedicated query trait (`IsSpeculatable`) parallel to
`IsPure`, plus derive support and wrapper propagation. The primary motivation is
to unblock optimization legality for movement and speculation-sensitive passes,
especially for arithmetic where `pure` is not sufficient to distinguish
trapping from non-trapping operations.

## Motivation

- Problem: Today Kirin only exposes `is_pure()`, which cannot represent "no side
  effects, but may trap". This makes transforms either over-conservative or
  unsound.
- Why now: RFC 0002 (`kirin-arith`) explicitly needs to mark `div`/`rem` as
  pure but not speculatable.
- Stakeholders:
  - `kirin-ir` trait consumers and pass authors
  - `kirin-derive*` maintainers
  - dialect authors (`kirin-arith` first)
  - parser/printer derive users (attribute surface)

## Goals

- Represent speculative safety as a first-class statement property.
- Keep property ergonomics symmetric with `pure`, `constant`, `terminator`.
- Support both global defaults (`#[kirin(speculatable)]` on dialect type) and
  per-statement overrides.
- Ensure wrapper variants/structs forward `is_speculatable()` consistently.

## Non-goals

- Defining a full effect system.
- Proving speculatability automatically from semantics.
- Pass scheduling policy (this RFC only defines the signal, not transforms).

## Guide-level Explanation

Dialect authors can annotate operations that are safe to execute even when their
result is unused:

```rust
#[derive(Dialect)]
#[kirin(fn, type = T)]
pub enum Arith<T: CompileTimeValue + Default> {
    #[kirin(pure, speculatable)]
    Add { /* ... */ },
    #[kirin(pure, speculatable)]
    Mul { /* ... */ },
    #[kirin(pure)]
    Div { /* may trap on zero */ },
}
```

Optimization passes can query both properties:

```rust
if stmt.definition(stage).is_pure() && stmt.definition(stage).is_speculatable() {
    // Safe to speculate
}
```

Interpretation:

- `pure`: no side effects.
- `speculatable`: safe to execute even if the result is unused.

`speculatable` is intentionally separate from `pure` because these properties
are related but not equivalent in practice.

## Reference-level Explanation

### API and syntax changes

- Add `speculatable` to `#[kirin(...)]` parsing in
  `crates/kirin-derive-core/src/ir/attrs.rs`.
- Update unknown-attribute diagnostics in
  `crates/kirin-derive-core/src/misc.rs`.
- Add trait in `crates/kirin-ir/src/language.rs`:

```rust
pub trait IsSpeculatable {
    fn is_speculatable(&self) -> bool;
}
```

- Update `Dialect` to require `IsSpeculatable` as a supertrait, alongside the
  existing property traits.
- Extend derive generation in `crates/kirin-derive` and
  `crates/kirin-derive-dialect/src/property/*` with a new property kind
  mirroring `IsPure`.
- Add derive-time validation: `#[kirin(speculatable)]` requires
  `#[kirin(pure)]` at the same scope (global or statement-level effective value).
- Wrapper behavior follows existing property rules:
  - wrapper variant/struct delegates to wrapped type
  - non-wrapper uses `global || local` composition for enums
  - struct types use global value only

### Semantics and invariants

- `is_speculatable()` returning `true` means evaluating the statement early or
  conditionally-elided is semantically safe.
- `speculatable => pure` is a required invariant and is enforced by derive
  validation.
- `false` is conservative and means "do not speculate unless a pass proves
  stronger facts out-of-band."
- This property is compile-time metadata on statement definitions; it is not
  persisted as per-instance runtime IR state.
- Wrapper forwarding must preserve the wrapped definition's property.

### Crate impact matrix

| crate | impact | tests to update |
| --- | --- | --- |
| `kirin-ir` | add `IsSpeculatable` trait and make it a `Dialect` supertrait | trait/compile tests |
| `kirin-derive-core` | parse/store `speculatable` attr | parser unit tests + snapshots |
| `kirin-derive-dialect` | add property generation for speculatable | property tests/snapshots |
| `kirin-derive` | export derive macro for `IsSpeculatable` + include in `Dialect` derive flow | derive tests |
| `kirin-arith` | annotate ops (`div`/`rem` false, others true) | dialect property tests |

## Drawbacks

- Adds another property surface area and another axis pass authors must reason about.
- Making `IsSpeculatable` a `Dialect` supertrait is a breaking API change for
  manual `Dialect` implementors.
- Property misuse (marking unsafe ops as speculatable) can still cause
  miscompilations; review discipline is required.

## Rationale and Alternatives

### Proposed approach rationale

- Matches existing Kirin property model (`pure`, `constant`, `terminator`) with
  minimal conceptual overhead.
- Keeps legality queries local and explicit in pass code.
- Enables RFC 0002 semantics without overloading `pure`.

### Alternative A

- Description: Encode speculatability only in pass-local whitelists.
- Pros: no derive or trait changes.
- Cons: duplicated policy across passes, drift risk, poor discoverability.
- Reason not chosen: Kirin already has statement properties; this would bypass
  the existing architecture.

### Alternative B

- Description: Fold speculatability into `pure`.
- Pros: fewer property names.
- Cons: cannot represent "pure but may trap" (`div`, `rem`).
- Reason not chosen: loses essential semantic distinction needed by optimizers.

## Prior Art

- LLVM/MLIR distinction between side-effect freedom and speculation legality.
- Existing Kirin property derivation design in `kirin-derive*`.

## Backward Compatibility and Migration

- Breaking changes: yes. Manual `Dialect` implementations must provide
  `IsSpeculatable`.
- Migration steps:
  1. update derive crates and traits
  2. update manual dialect implementations with `is_speculatable()`
  3. run derives; most dialects default to `false` unless annotated
  4. fix any derive-time `speculatable => pure` violations
  5. annotate known-safe operations incrementally
- Compatibility strategy: default `false` behavior preserves safety.
  Invalid combinations (`speculatable` without `pure`) are compile-time derive
  errors.

## How to Teach This

- Add a short property matrix in docs: `constant`, `pure`, `speculatable`,
  `terminator`.
- Update arithmetic examples to show `div`/`rem` as non-speculatable.
- Provide one pass example that requires both `pure` and `speculatable`.

## Reference Implementation Plan

1. Add `speculatable` to derive-core attrs and diagnostics.
2. Add `IsSpeculatable` trait to `kirin-ir`.
3. Generate the new property impl in derive crates, including wrappers.
4. Add tests analogous to `dialect_properties` for all property combinations.
5. Annotate `kirin-arith` once RFC 0002 implementation lands.

### Acceptance Criteria

- [ ] `#[kirin(speculatable)]` parses on type/statement attributes.
- [ ] `Dialect`-derived types expose `is_speculatable()` with wrapper forwarding.
- [ ] derive rejects `speculatable` configurations that are not also `pure`.
- [ ] Property tests cover enum variants, structs, and wrappers.

### Tracking Plan

- Tracking issue: TBD
- Implementation PRs: TBD
- Follow-up tasks:
  - Adopt in `kirin-arith`
  - Add pass-level examples/docs

## Unresolved Questions

- Should `IsSpeculatable` become a required `Dialect` supertrait immediately, or should migration use a staged rollout to reduce breakage for manual `Dialect` implementors?
- While this RFC is still draft, what policy should dependent RFCs follow when they reference speculatability (intent-only annotations vs blocking on acceptance)?

## Future Possibilities

- Additional effect properties (e.g., `nothrow`, `readonly`) if/when memory
  operations are modeled.
- Verification passes that infer/validate property consistency.

## Revision Log

| Date | Change |
| --- | --- |
| 2026-02-09T04:05:27.219042Z | RFC created from template |
| 2026-02-09 | Replaced template with concrete proposal for `#[kirin(speculatable)]` |
| 2026-02-09 | Decision: `#[kirin(speculatable)]` now requires `#[kirin(pure)]` |
| 2026-02-09 | Decision: `IsSpeculatable` is required as a `Dialect` supertrait |
