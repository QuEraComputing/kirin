+++
rfc = "0007"
title = "Math Dialect"
status = "Draft"
agents = ["codex"]
authors = ["Roger-luo <code@rogerluo.dev>"]
created = "2026-02-09T04:05:32.90461Z"
last_updated = "2026-02-09T04:05:32.90461Z"
dependencies = ["0002", "0003", "0006"]
+++

# RFC 0007: Math Dialect

## Summary

Introduce a reusable `kirin-math` dialect for transcendental and advanced
floating-point operations that are intentionally out of scope for `kirin-arith`.
The dialect provides common unary/binary math operations (e.g., `sin`, `cos`,
`exp`, `log`, `sqrt`, `pow`) with consistent text format and derive-based
parser/printer support. This creates a clean separation: `arith` for primitive
numeric ops, `math` for transcendental numerics.

## Motivation

- Problem: RFC 0002 deferred transcendental operations; users currently need
  ad-hoc per-language math statement definitions.
- Why now: math ops are a common next step after arithmetic and casts.
- Stakeholders:
  - language authors targeting scientific/numeric domains
  - optimization/lowering passes handling math intrinsics
  - parser/printer maintainers

## Goals

- Provide a standard math dialect crate with stable operation names.
- Keep composability with existing Kirin dialect patterns.
- Separate transcendental semantics from integer/float primitive arithmetic.
- Enable canonical parser/printer roundtrip behavior for math ops.

## Non-goals

- Full libm parity on day one.
- Vectorized/SIMD math operations.
- Domain-specific approximations or target-intrinsic lowering policy.

## Guide-level Explanation

Example syntax:

```text
%r = sin %x -> f64
%r = cos %x -> f64
%r = exp %x -> f64
%r = log %x -> f64
%r = sqrt %x -> f64
%r = pow %base, %exp -> f64
```

The math dialect is usually composed with `arith`, `cast`, and control-flow
operations.

## Reference-level Explanation

### API and syntax changes

Add a new crate:

- `crates/kirin-math/src/lib.rs`

Proposed operation surface (initial set):

- Unary: `abs`, `sqrt`, `exp`, `log`, `sin`, `cos`
- Binary: `pow`

The v1 scope is intentionally small. Additional operations (for example `tan`,
`atan2`, and others) are follow-up extensions.

Illustrative enum shape:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, HasParser, PrettyPrint)]
#[kirin(fn, type = T)]
pub enum Math<T: CompileTimeValue + Default> {
    #[chumsky(format = "{result:name} = sin {operand} -> {result:type}")]
    Sin { operand: SSAValue, result: ResultValue, #[kirin(default)] marker: std::marker::PhantomData<T> },
    // ... remaining unary/binary ops
}
```

### Semantics and invariants

- Math ops target floating-point-like types; verifier defines admissible types.
- Operation semantics are math-intrinsic semantics, not host-language API calls.
- Property intent:
  - math ops are pure
  - speculatability depends on final NaN/exception policy and RFC 0003

### Crate impact matrix

| crate | impact | tests to update |
| --- | --- | --- |
| `kirin-math` | new dialect crate | new crate tests |
| `kirin-chumsky` / `kirin-prettyless` | parser/printer exercised by math formats | roundtrip snapshots |
| `kirin-arith` | composition examples with primitive arithmetic | integration tests |
| `kirin-test-utils` | optional numeric language fixtures | helper additions if reused |

## Drawbacks

- Adds another dialect crate and maintenance surface.
- Floating-point semantics are subtle; verifier/docs burden is non-trivial.
- Scope control is needed to avoid immediate operation explosion.

## Rationale and Alternatives

### Proposed approach rationale

- Mirrors MLIR-style separation between primitive arithmetic and transcendental
  math while keeping Kirin's generic type parameter model.
- Gives downstream users a reusable default instead of repeatedly reinventing
  math op sets.

### Alternative A

- Description: keep all math ops inside `kirin-arith`.
- Pros: fewer crates.
- Cons: `arith` becomes broad and less coherent.
- Reason not chosen: clear boundaries improve composability and maintenance.

### Alternative B

- Description: define no standard math dialect; let each language implement its own.
- Pros: maximal flexibility.
- Cons: duplicated syntax and inconsistent optimization assumptions.
- Reason not chosen: loses the ecosystem-level reuse Kirin dialects are meant to provide.

## Prior Art

- MLIR `math` dialect as reference for scope separation.
- LLVM/libm intrinsic families as naming and semantics reference points.

## Backward Compatibility and Migration

- Breaking changes: none; additive new crate.
- Migration steps:
  1. add `kirin-math` dependency
  2. wrap `Math<T>` in language enum
  3. replace custom transcendental ops incrementally
- Compatibility strategy: custom dialects can coexist during migration.

## How to Teach This

- Teach `arith` vs `math` boundary with two short examples.
- Document the initial op set and any verifier restrictions.
- Provide one composed language example using `arith + cast + math + cf`.

## Reference Implementation Plan

1. Create `crates/kirin-math` with initial unary/binary operations.
2. Add parser/printer roundtrip tests for each operation.
3. Add integration test with mixed arithmetic and math expressions.
4. Document semantics and composition examples.

### Acceptance Criteria

- [ ] initial math operation set parses and prints.
- [ ] dialect composes with existing Kirin dialects via wrappers.
- [ ] roundtrip tests cover unary and binary math ops.
- [ ] semantics notes clearly define type expectations.

### Tracking Plan

- Tracking issue: TBD
- Implementation PRs: TBD
- Follow-up tasks:
  - extend operation set if required by real users
  - add canonicalization/lowering examples

## Unresolved Questions

- For each v1 op (`abs`, `sqrt`, `exp`, `log`, `sin`, `cos`, `pow`), what are the precise semantics for domain edges and special values (`NaN`, `+/-inf`, signed zero)?
- Which math ops, if any, are speculatable under RFC 0003 once NaN/exception policy is fixed?
- Should verifier rules permit only floating-point-like types for all current ops, or allow future extension hooks for non-standard numeric types in v1?

## Future Possibilities

- Optional fast-math flags or approximation modes.
- Vector math dialect extension.

## Revision Log

| Date | Change |
| --- | --- |
| 2026-02-09T04:05:32.90461Z | RFC created from template |
| 2026-02-09 | Replaced template with concrete `kirin-math` proposal |
| 2026-02-09 | Decision: v1 math op set is the small core (`sin`, `cos`, `exp`, `log`, `sqrt`, `pow`) |
| 2026-02-09 | Decision: `abs` remains in `kirin-math` |
