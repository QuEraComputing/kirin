+++
rfc = "0006"
title = "Arithmetic Cast Operations"
status = "Draft"
agents = ["codex"]
authors = ["Roger-luo <code@rogerluo.dev>"]
created = "2026-02-09T04:05:32.427674Z"
last_updated = "2026-02-09T04:05:32.427674Z"
dependencies = ["0002", "0003"]
+++

# RFC 0006: Arithmetic Cast Operations

## Summary

Introduce a dedicated cast dialect (`kirin-cast`) for explicit numeric
conversions between integer and floating-point types. The RFC proposes a single
`cast` statement carrying a `CastKind` enum that encodes conversion intent
(`zext`, `sext`, `trunc`, `sitofp`, `uitofp`, `fptosi`, `fptoui`, `fpext`,
`fptrunc`, `bitcast`). This keeps operation count small while preserving fully
explicit semantics needed for verification and optimization. Float-to-int casts
(`fptosi`/`fptoui`) use deterministic saturating semantics.

## Motivation

- Problem: RFC 0002 intentionally excluded casts; without them, many real-world
  arithmetic programs cannot express type transitions.
- Why now: cast semantics influence legality and optimization across arithmetic,
  bitwise, and math dialects.
- Stakeholders:
  - `kirin-arith` users needing mixed-type programs
  - parser/printer maintainers
  - optimization and verification pass authors

## Goals

- Provide explicit conversion operations with no implicit coercions.
- Keep operation surface compact and easy to compose.
- Support both signed and unsigned integer conversion paths.
- Keep dialect generic over type system (`T: CompileTimeValue + Default`).

## Non-goals

- High-level language coercion rules.
- Vector/tensor conversion semantics.
- User-defined cast operators with custom runtime behavior.

## Guide-level Explanation

Casts are explicit statements. Example syntax:

```text
%r = cast zext %x -> i64
%r = cast sext %x -> i64
%r = cast trunc %x -> i32
%r = cast sitofp %x -> f64
%r = cast uitofp %x -> f64
%r = cast fptosi %x -> i32
%r = cast fptoui %x -> u32
%r = cast fpext %x -> f64
%r = cast fptrunc %x -> f32
%r = cast bitcast %x -> i32
```

The verifier checks that source/target type pairs are valid for each `CastKind`.

Instruction meanings:

- `zext`: zero-extend unsigned integer bits to a wider integer type.
- `sext`: sign-extend signed integer bits to a wider integer type.
- `trunc`: truncate integer to a narrower integer type (drop high bits).
- `sitofp`: convert signed integer to floating-point.
- `uitofp`: convert unsigned integer to floating-point.
- `fptosi`: convert floating-point to signed integer using saturating semantics.
- `fptoui`: convert floating-point to unsigned integer using saturating semantics.
- `fpext`: widen floating-point precision (e.g., `f32 -> f64`).
- `fptrunc`: narrow floating-point precision (e.g., `f64 -> f32`).
- `bitcast`: reinterpret bit pattern without numeric conversion (same bitwidth).

## Reference-level Explanation

### API and syntax changes

Add a new crate:

- `crates/kirin-cast/src/lib.rs`

Proposed dialect definition:

```rust
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, HasParser, PrettyPrint)]
pub enum CastKind {
    #[chumsky(format = "zext")] ZExt,
    #[chumsky(format = "sext")] SExt,
    #[chumsky(format = "trunc")] Trunc,
    #[chumsky(format = "sitofp")] SIToFP,
    #[chumsky(format = "uitofp")] UIToFP,
    #[chumsky(format = "fptosi")] FPToSI,
    #[chumsky(format = "fptoui")] FPToUI,
    #[chumsky(format = "fpext")] FPExt,
    #[chumsky(format = "fptrunc")] FPTrunc,
    #[chumsky(format = "bitcast")] Bitcast,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, HasParser, PrettyPrint)]
#[kirin(fn, type = T)]
pub enum Cast<T: CompileTimeValue + Default> {
    #[chumsky(format = "{result:name} = cast {kind} {operand} -> {result:type}")]
    Cast {
        kind: CastKind,
        operand: SSAValue,
        result: ResultValue,
        #[kirin(default)]
        marker: std::marker::PhantomData<T>,
    },
}
```

### Semantics and invariants

- `CastKind` fully determines conversion intent.
- Source and target types are validated by a verifier pass.
- `bitcast` requires bitwidth-preserving reinterpretation compatibility.
- Per-instruction semantics:
  - `zext`: int -> wider int, high bits filled with zero.
  - `sext`: int -> wider int, high bits filled with sign bit.
  - `trunc`: int -> narrower int, high bits discarded.
  - `sitofp`: signed int -> float conversion; exact when representable, otherwise rounded per IEEE conversion rules.
  - `uitofp`: unsigned int -> float conversion; exact when representable, otherwise rounded per IEEE conversion rules.
  - `fptosi`: float -> signed int with saturating semantics:
    - finite values: truncate toward zero, then clamp to target range
    - `NaN`: `0`
    - `+inf`: target max
    - `-inf`: target min
  - `fptoui`: float -> unsigned int with saturating semantics:
    - finite values: truncate toward zero, then clamp to `[0, target_max]`
    - `NaN`: `0`
    - `+inf`: target max
    - `-inf`: `0`
  - `fpext`: float -> wider float precision.
  - `fptrunc`: float -> narrower float precision using IEEE rounding.
  - `bitcast`: reinterpret bits with equal total bitwidth; source/target type compatibility enforced by verifier.
- Purity/speculatability:
  - all cast operations are pure
  - all cast operations are speculatable under RFC 0003 because they are total
    and non-trapping with the saturating float-to-int policy

### Crate impact matrix

| crate | impact | tests to update |
| --- | --- | --- |
| `kirin-cast` | new dialect crate with `CastKind` and `Cast<T>` | new crate tests |
| `kirin-arith` | integration usage examples/tests | integration tests |
| `kirin-chumsky` / `kirin-prettyless` | exercised by new format patterns | roundtrip snapshots |
| `kirin-test-utils` | optional cast-heavy language fixture | helper additions if reused |

## Drawbacks

- Introduces a new crate and verifier burden for many source/target pairs.
- A single `cast` opcode with enum argument is slightly less pattern-match
  ergonomic than many dedicated op variants.
- Numeric corner cases (NaN, infinity, out-of-range) must be specified precisely.

## Rationale and Alternatives

### Proposed approach rationale

- Keeps API compact while preserving explicit semantics.
- Avoids variant explosion in derive-generated boilerplate.
- Aligns with Kirin preference for small imported name sets and clear enums.

### Alternative A

- Description: model each conversion as a separate op (`zext`, `sext`, etc.).
- Pros: simpler per-op matching and potentially clearer rewrites.
- Cons: larger dialect surface and duplicated struct fields.
- Reason not chosen: `CastKind` captures the same intent with less boilerplate.

### Alternative B

- Description: infer cast semantics purely from source/target types using one generic `cast`.
- Pros: minimal syntax.
- Cons: ambiguous for signedness-sensitive cases and less explicit IR.
- Reason not chosen: explicitness is required for verifier and optimizer clarity.

## Prior Art

- MLIR `arith`/`math` cast families (`sitofp`, `fptosi`, etc.).
- LLVM conversion instruction taxonomy.

## Backward Compatibility and Migration

- Breaking changes: none; additive new dialect.
- Migration steps:
  1. add `kirin-cast` dependency
  2. wrap `Cast<T>` in language enum
  3. replace ad-hoc conversion ops with `cast kind` syntax
- Compatibility strategy: incremental adoption alongside existing custom casts.

## How to Teach This

- Start from "all conversions are explicit" rule.
- Show quick mapping table from common conversion intents to `CastKind`.
- Add one example mixing `arith + cast + cf` in docs/tests.

## Reference Implementation Plan

1. Create `crates/kirin-cast` with `CastKind` and `Cast<T>`.
2. Add parser/printer roundtrip tests for each `CastKind`.
3. Add verifier test matrix for valid/invalid source-target combinations.
4. Integrate into example language and update docs.

### Acceptance Criteria

- [ ] all `CastKind` variants parse/print correctly.
- [ ] verifier enforces source/target legality by kind.
- [ ] `fptosi`/`fptoui` saturating behavior is specified and tested (including `NaN` and infinities).
- [ ] roundtrip tests cover integer and floating conversions.
- [ ] integration test composes with `kirin-arith`.

### Tracking Plan

- Tracking issue: TBD
- Implementation PRs: TBD
- Follow-up tasks:
  - optional fixed-point casts in a later RFC
  - canonicalization folds (`trunc(zext(x))`, etc.)

## Unresolved Questions

- If `Cast<T>` remains generic, what trait-level type introspection API is required so the verifier can check signedness/category/bitwidth legality for each `CastKind`?
- Should v1 scope narrow to `Cast<ArithType>` first, then generalize once type introspection infrastructure exists?
- What is the exact v1 compatibility matrix for `bitcast` (integer<->integer only, integer<->float of equal bitwidth, float<->float only)?

## Future Possibilities

- Vector/tensor cast extensions.
- Fast-math style cast flags if needed by lowering pipelines.

## Revision Log

| Date | Change |
| --- | --- |
| 2026-02-09T04:05:32.427674Z | RFC created from template |
| 2026-02-09 | Replaced template with concrete cast dialect proposal |
| 2026-02-09 | Decision: `fptosi`/`fptoui` use deterministic saturating semantics |
| 2026-02-09 | Added explicit per-instruction semantics for all `CastKind` variants |
| 2026-02-09 | Decision: `bitcast` remains in `kirin-cast` |
