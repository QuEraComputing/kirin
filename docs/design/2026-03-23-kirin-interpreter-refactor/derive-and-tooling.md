# Derive Package And Toolkit Design

## Separate Derive Package

The new runtime should have its own derive package:

- `kirin-derive-interpreter-2`

It should not extend `kirin-derive-interpreter` in place. The new interpreter
has different core traits and a different execution boundary, so a separate
derive crate keeps the macro API aligned with v2 instead of carrying
compatibility debt from the old continuation-era model.

## Macro Surface

The v2 macro surface should be:

- `#[derive(Interpretable)]`
- `#[derive(ConsumeResult)]`
- `#[derive(CallableBody)]`
- `#[derive(SSACFGCallableBody)]`

These names intentionally preserve the generic semantic names where they still
fit, but they do not carry forward old runtime-specific names such as
`CallSemantics` or `SSACFGRegion`.

## Attribute Model

The derive-specific attribute model should be:

- reuse `#[wraps]`
- reuse `#[callable]`
- reuse `#[interpret(...)]` for derive-local options, including interpreter
  crate path override
- add `#[body]` for concrete body-specific derives

`#[kirin(crate = ...)]` remains the IR crate path. `#[interpret(crate = ...)]`
is the interpreter crate path override for the new derive family.

## Forwarding Rules

Forwarding rules should stay strict and predictable:

- `Interpretable` derive is wrapper-only and forwards all `#[wraps]`
- `ConsumeResult` derive is wrapper-only and forwards all `#[wraps]`
- `CallableBody` derive is wrapper-only and forwards only `#[wraps]` selected
  by `#[callable]`
- `SSACFGCallableBody` uses the same `#[callable]` selection rule for wrapper
  forwarding

For concrete `SSACFGCallableBody` structs:

- exactly one field must be marked `#[body]`
- that field must be `Region`
- the derive should hard-error otherwise

This keeps:

- `#[wraps]` as the marker of semantic equivalence
- `#[callable]` as the marker of callable-body forwarding
- `#[body]` as the generic body marker reusable by future body derives

## Shared Toolkit Support

`kirin-derive-interpreter-2` should not reimplement forwarding logic ad hoc.
Instead, `kirin-derive-toolkit` should grow shared support for:

- generic wrapper-forwarding templates
- selector-policy configuration
- `#[body]` field lookup and validation

The forwarding template family should be generic rather than
interpreter-specific. It should support:

- all-wrapper forwarding
- selector-gated forwarding
- configurable behavior for non-forwarding cases
- configurable selector policy, including strict explicit-selection mode for
  v2 derives instead of inheriting old backward-compat fallback behavior

For `kirin-derive-interpreter-2`, the mapping should be:

- `Interpretable`: all wrappers, require all
- `ConsumeResult`: all wrappers, require all
- `CallableBody`: selected wrappers via `#[callable]`, explicit-only
- `SSACFGCallableBody`: same forwarding config plus concrete `#[body] Region`
  validation

## Darling And Layout Interaction

The new toolkit/template design should make attribute handling explicit.

Recommended split:

- `#[interpret(...)]` is darling-parsed
- `#[wraps]`, `#[callable]`, and `#[body]` remain bare helper attributes

For v2 derives, `kirin-derive-toolkit` should add an interpreter-oriented
layout/context that precomputes normalized metadata:

- global attrs:
  - `#[interpret(crate = ...)]`
- statement attrs:
  - wrapper selection
  - `callable: bool`
- field attrs:
  - `body: bool`

The new shared templates should primarily consume this normalized metadata
rather than re-reading raw attributes ad hoc.

The generic toolkit API can still allow raw-attribute fallback hooks for other
derives or backward compatibility, but `kirin-derive-interpreter-2` should use
the pre-parsed path by default.

## Derive And Toolkit Testing

`kirin-derive-interpreter-2` should have focused test coverage for:

- generated-code snapshots or token-based tests
- compile-pass cases for happy paths
- compile-fail cases for invalid combinations such as:
  - missing `#[wraps]` on forwarding derives
  - missing `#[callable]` on callable-body forwarding derives
  - invalid `#[body]` usage on `SSACFGCallableBody`

`kirin-derive-toolkit` should also gain targeted tests for the shared helpers,
especially:

- explicit-only selector policy
- backward-compatible fallback policy where still needed by older derives
- `#[body]` field lookup and error reporting
