+++
rfc = "0005"
title = "Bitwise And Shift Dialect"
status = "Draft"
agents = ["codex"]
authors = ["Roger-luo <code@rogerluo.dev>"]
created = "2026-02-09T04:05:32.043982Z"
last_updated = "2026-02-09T04:05:32.043982Z"
dependencies = ["0002", "0003"]
+++

# RFC 0005: Bitwise And Shift Dialect

## Summary

Introduce a reusable `kirin-bitwise` dialect for integer bitwise and shift
operations: `and`, `or`, `xor`, `not`, `shl`, and `shr`. The dialect follows
Kirin's generic design (`Bitwise<T>`) so languages can use their own type
systems while still sharing operation definitions, parser/printer behavior, and
optimization metadata. This RFC keeps the dialect focused on scalar integer
bit manipulation and intentionally excludes arithmetic, casts, and
transcendental math.

## Motivation

- Problem: Languages built on Kirin repeatedly redefine bitwise ops and their
  syntax, causing duplicated implementations and inconsistent semantics.
- Why now: RFC 0002 explicitly deferred bitwise/shift operations to a future
  RFC; arithmetic and bitwise are typically adopted together.
- Stakeholders:
  - dialect authors composing low-level languages
  - `kirin-chumsky` / `kirin-prettyless` users
  - pass authors relying on purity/speculatability metadata

## Goals

- Provide a standard dialect crate for scalar bitwise and shift operations.
- Keep operation names and format style aligned with RFC 0002 (`arith`).
- Preserve type-generic design (`T: CompileTimeValue + Default`).
- Mark non-trapping ops as pure and, once RFC 0003 lands, speculatable.

## Non-goals

- Arithmetic operations (already covered by RFC 0002).
- Bit counting and bit-twiddling intrinsics (`popcnt`, `clz`, `ctz`, rotates).
- Vector/SIMD bitwise operations.
- Integer/float casts (RFC 0006).

## Guide-level Explanation

The bitwise dialect composes into a language like other Kirin dialects:

```rust
#[derive(Dialect)]
#[kirin(type = MyType)]
pub enum MyLanguage {
    #[kirin(wraps)]
    Bitwise(Bitwise<MyType>),
    #[kirin(wraps)]
    Arith(Arith<MyType>),
}
```

Example text format:

```text
%r = and %a, %b -> i32
%r = or %a, %b -> u64
%r = xor %a, %b -> i8
%r = not %a -> i16
%r = shl %a, %b -> u32
%r = shr %a, %b -> i32
```

Bitwise ops are intended for integer-like types only. Type legality is checked
by a verifier pass, not by parser shape.

## Reference-level Explanation

### API and syntax changes

Add a new crate:

- `crates/kirin-bitwise/src/lib.rs`

Proposed dialect surface:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, HasParser, PrettyPrint)]
#[kirin(fn, type = T)]
pub enum Bitwise<T: CompileTimeValue + Default> {
    #[chumsky(format = "{result:name} = and {lhs}, {rhs} -> {result:type}")]
    And { lhs: SSAValue, rhs: SSAValue, result: ResultValue, #[kirin(default)] marker: std::marker::PhantomData<T> },

    #[chumsky(format = "{result:name} = or {lhs}, {rhs} -> {result:type}")]
    Or { lhs: SSAValue, rhs: SSAValue, result: ResultValue, #[kirin(default)] marker: std::marker::PhantomData<T> },

    #[chumsky(format = "{result:name} = xor {lhs}, {rhs} -> {result:type}")]
    Xor { lhs: SSAValue, rhs: SSAValue, result: ResultValue, #[kirin(default)] marker: std::marker::PhantomData<T> },

    #[chumsky(format = "{result:name} = not {operand} -> {result:type}")]
    Not { operand: SSAValue, result: ResultValue, #[kirin(default)] marker: std::marker::PhantomData<T> },

    #[chumsky(format = "{result:name} = shl {lhs}, {rhs} -> {result:type}")]
    Shl { lhs: SSAValue, rhs: SSAValue, result: ResultValue, #[kirin(default)] marker: std::marker::PhantomData<T> },

    #[chumsky(format = "{result:name} = shr {lhs}, {rhs} -> {result:type}")]
    Shr { lhs: SSAValue, rhs: SSAValue, result: ResultValue, #[kirin(default)] marker: std::marker::PhantomData<T> },
}
```

### Semantics and invariants

- `and` / `or` / `xor` / `not` are bitwise operations over integer-like types.
- `shl` and `shr` are shift operations over integer-like types.
- `shr` is a single operation whose semantics are determined by operand/result
  type:
  - signed integer types use arithmetic right shift
  - unsigned integer types use logical right shift
- Shift count (`rhs`) must have the same integer type (including width and
  signedness) as `lhs`/result.
- Operands and result must satisfy verifier-defined type compatibility.
- Property intent:
  - `and`/`or`/`xor`/`not` are pure and speculatable.
  - shift operations are intended to be pure; speculatability depends on final
    shift-count semantics.

### Crate impact matrix

| crate | impact | tests to update |
| --- | --- | --- |
| `kirin-bitwise` | new dialect crate with parser/printer derives | new crate tests |
| `kirin-derive*` | no new macro features required | none |
| `kirin-chumsky` / `kirin-prettyless` | exercised by new dialect snapshots | integration snapshots |
| `kirin-test-utils` | optional shared helpers for bitwise test languages | new helper exports if reused |

## Drawbacks

- Adds one more dialect crate to maintain.
- Shift semantics need precise definition to avoid optimizer ambiguity.
- Some low-level operations remain out of scope and need follow-up RFCs.

## Rationale and Alternatives

### Proposed approach rationale

- Mirrors RFC 0002 style, so users get consistent operation naming and format.
- Keeps arith and bitwise concerns separate while still composable.
- Reduces duplicate dialect code across downstream projects.

### Alternative A

- Description: add bitwise ops directly to `kirin-arith`.
- Pros: fewer crates.
- Cons: larger, less focused dialect; weaker modularity.
- Reason not chosen: bitwise and arithmetic have different extension cadence.

### Alternative B

- Description: split into several micro-dialects (`kirin-bitwise`, `kirin-shift`, etc.).
- Pros: maximal granularity.
- Cons: over-fragmentation and boilerplate for common use cases.
- Reason not chosen: one focused crate is a better default boundary.

## Prior Art

- MLIR `arith` shift/bitwise operations (operation-level inspiration).
- LLVM integer bitwise instruction set (semantics vocabulary).

## Backward Compatibility and Migration

- Breaking changes: none; additive new crate.
- Migration steps:
  1. add `kirin-bitwise` dependency
  2. wrap `Bitwise<T>` into dialect enum
  3. update text snapshots and parser tests
- Compatibility strategy: incremental adoption; existing custom bitwise dialects
  can coexist.

## How to Teach This

- Show a minimal language composed from `Arith + Bitwise + Constant`.
- Document that this RFC covers scalar integer bit ops only.
- Provide one pass example (e.g., simplify `x & x -> x`) gated on purity.

## Reference Implementation Plan

1. Create `crates/kirin-bitwise` with `Bitwise<T>` enum and derives.
2. Add parser/printer roundtrip tests for each op and representative types.
3. Add integration test composing bitwise with control flow/constants.
4. Annotate operation properties after RFC 0003 property support lands.

### Acceptance Criteria

- [ ] `and`, `or`, `xor`, `not`, `shl`, `shr` parse and print with result type annotations.
- [ ] dialect composes with other Kirin dialects through `#[kirin(wraps)]`.
- [ ] roundtrip `print -> parse -> print` passes for all operations.
- [ ] semantics notes document shift behavior unambiguously.

### Tracking Plan

- Tracking issue: TBD
- Implementation PRs: TBD
- Follow-up tasks:
  - optional extended bit-twiddling dialect (`popcnt`, rotates)
  - verifier rules for shift-count legality

## Unresolved Questions

- What are the exact semantics for out-of-range shift counts in IR (`rhs < 0`, `rhs >= bitwidth`)?
- Given the finalized shift-count semantics, should `shl` and `shr` be marked speculatable by default under RFC 0003?
- Should shift-count typing stay strict (`rhs` has same type as `lhs`/result) or allow a wider integer-typed `rhs` with verifier rules?

## Future Possibilities

- Separate extension RFC for rotate and bit counting operations.
- Canonicalization/folding rules for common bitwise identities.

## Revision Log

| Date | Change |
| --- | --- |
| 2026-02-09T04:05:32.043982Z | RFC created from template |
| 2026-02-09 | Replaced template with concrete `kirin-bitwise` dialect proposal |
| 2026-02-09 | Decision: keep single `shr`; signedness is determined by type |
| 2026-02-09 | Decision: shift count `rhs` must match `lhs` type/width |
