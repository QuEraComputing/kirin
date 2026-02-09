+++
rfc = "0002"
title = "Generic arith dialect"
status = "Draft"
authors = ["Roger-luo <code@rogerluo.dev>"]
agents = ["claude opus"]
created = "2026-02-09T01:38:49.788023Z"
last_updated = "2026-02-09T02:00:00.000000Z"
dependencies = ["0001"]
+++

# RFC 0002: Generic arith dialect

## Summary

Introduce a generic arithmetic dialect (`kirin-arith`) that provides unified arithmetic statements (add, sub, mul, div, rem, neg), parameterized over the type system. Operations are type-generic — the result type determines semantics, with no separate integer/float opcodes. Additionally, define a built-in `ArithType` enum mirroring Rust's numeric types (signed/unsigned integers, floats) and a corresponding `ArithValue` enum that can serve as a concrete type system for languages needing standard arithmetic. The dialect is generic over `T: CompileTimeValue + Default` so it composes with any language's type system, while `ArithType`/`ArithValue` provide a batteries-included option.

Comparison operations (`cmp`, `is_nan`) and `bool` type are deliberately excluded from this dialect and will be covered by a separate `kirin-cmp` RFC, mirroring Rust's separation of arithmetic traits (`Add`/`Sub`/...) from comparison traits (`PartialEq`/`PartialOrd`).

## Motivation

- Problem: Every language built on Kirin that needs arithmetic must define its own add/sub/mul/div statements and numeric types from scratch. This leads to duplicated dialect definitions, inconsistent naming, and repeated parser/printer work.
- Why now: With the dialect composition model stable and function parsing landed (RFC 0001), arithmetic is the most common missing building block for real programs. The `kirin-arith` crate directory already exists as a placeholder.
- Stakeholders:
  - Dialect authors composing languages with arithmetic
  - `kirin-chumsky` / `kirin-prettyless` (parser/printer support)
  - `kirin-constant` (constants of `ArithType` values)
  - Example and test code in `tests/` and `example/`

## Goals

- Provide a reusable set of binary arithmetic operations: add, sub, mul, div, rem. Each operation is type-generic — the result type determines semantics.
- Provide unary negation (signed integers and floats only, matching Rust's `Neg` trait).
- Define `ArithType` as a concrete numeric type enum mirroring Rust's primitive numeric types: signed integers (`I8`..`I128`), unsigned integers (`U8`..`U128`), and floats (`F32`, `F64`). No `bool` — that belongs in a separate comparison dialect.
- Define `ArithValue` as compile-time values mirroring `ArithType` one-to-one: `I8(i8)`/`U8(u8)`, ..., `F32`/`F64`.
- Keep the dialect generic over `T: CompileTimeValue + Default` so it works with custom type systems.
- Derive `Dialect`, `HasParser`, and `PrettyPrint` for all statement types. `ArithValue` uses a manual `HasParser` impl with heuristic parsing.
- Mark all arithmetic operations as pure (`#[kirin(pure)]`). Concrete overflow and division-by-zero behavior is abstract at the IR level — lowering passes determine whether to insert checks (debug mode) or allow wrapping (release mode).

## Non-goals

- Bitwise and shift operations (future `kirin-bitwise` dialect).
- Concrete overflow semantics (abstract at IR level; lowering decides debug-panic vs release-wrap).
- Vector/tensor types or element-wise operations.
- Type casting or conversion operations (future RFC).
- Transcendental math functions (sin, cos, exp, etc. — future `kirin-math` dialect).
- Comparison operations and `bool` type (separate `kirin-cmp` RFC).
- Separate signed/unsigned division opcodes — since `ArithType` carries signedness, the type determines whether `div`/`rem` use signed or unsigned semantics.

## Guide-level Explanation

### Using the dialect

The arith dialect provides common arithmetic as reusable IR statements. You compose it into your language like any other dialect:

```rust
use kirin_arith::{Arith, ArithType, ArithValue};
use kirin_constant::Constant;
use kirin_cf::ControlFlow;

#[derive(Dialect)]
#[kirin(type = ArithType)]
pub enum MyLanguage {
    #[kirin(wraps)]
    Arith(Arith<ArithType>),
    #[kirin(wraps)]
    Constant(Constant<ArithValue, ArithType>),
    #[kirin(wraps)]
    Cf(ControlFlow<ArithType>),
}
```

If your language has its own type system, just parameterize differently:

```rust
#[derive(Dialect)]
#[kirin(type = MyType)]
pub enum MyLanguage {
    #[kirin(wraps)]
    Arith(Arith<MyType>),
    // ...
}
```

### Adopting different language semantics

Because the statements are generic over the type parameter, the **same set of operations** can express arithmetic for different languages simply by swapping the type. The type determines the semantics — not the opcode.

For example, Python has very different arithmetic semantics from Rust: integers are arbitrary-precision (never overflow), there are no unsigned types, and `//` (floor division) rounds toward negative infinity instead of toward zero. Yet the same `Arith<T>` dialect handles both:

```rust
/// Python's numeric type system — just two types.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, HasParser, PrettyPrint)]
pub enum PythonNumType {
    #[chumsky(format = "int")]
    Int,     // arbitrary-precision integer
    #[chumsky(format = "float")]
    Float,   // always f64
}

#[derive(Dialect)]
#[kirin(type = PythonNumType)]
pub enum PythonLang {
    #[kirin(wraps)]
    Arith(Arith<PythonNumType>),  // same Add, Sub, Mul, Div, Rem, Neg
    // ...
}
```

The IR text is identical in structure — only the type annotations change:

```text
// Rust: fixed-width, may overflow (lowering decides behavior)
%r = add %a, %b -> i32

// Python: arbitrary-precision, never overflows
%r = add %a, %b -> int
```

The statement definition (`Add { lhs, rhs, result }`) is shared. The **lowering pass** for each language handles the semantic differences:

| Operation | Rust lowering | Python lowering |
| --- | --- | --- |
| `add` | Fixed-width addition (wrap or panic) | Arbitrary-precision addition (never overflows) |
| `div` | Truncation toward zero | Floor toward negative infinity |
| `rem` | Remainder (sign follows dividend) | Modulo (always non-negative for positive divisor) |
| `neg` | Two's complement (may overflow `i*::MIN`) | Arbitrary-precision (always succeeds) |

This means dialect authors write the statement types **once**, and every language built on Kirin reuses them. No duplicated `PythonAdd` / `RustAdd` / `JuliaAdd` definitions — just `Add<T>` parameterized over the type system.

### Text format

Arithmetic operations use a consistent format. The result type determines semantics — there are no separate `addi`/`addf` opcodes:

```text
%result = add %a, %b -> i32
%result = add %a, %b -> u32
%result = add %a, %b -> f64
%result = sub %a, %b -> i64
%result = mul %a, %b -> u16
%result = div %a, %b -> i32
%result = rem %a, %b -> i32
%result = neg %a -> i32
%result = div %a, %b -> f32
%result = neg %a -> f64
```

### ArithType

The built-in `ArithType` mirrors Rust's primitive numeric types (no `bool` — that belongs in the `kirin-cmp` dialect):

```text
i8           // signed 8-bit integer
i16          // signed 16-bit integer
i32          // signed 32-bit integer
i64          // signed 64-bit integer
i128         // signed 128-bit integer
u8           // unsigned 8-bit integer
u16          // unsigned 16-bit integer
u32          // unsigned 32-bit integer
u64          // unsigned 64-bit integer
u128         // unsigned 128-bit integer
f32          // 32-bit float
f64          // 64-bit float
```

The type carries signedness information, following Rust convention rather than MLIR's signless integers. This means `i32` and `u32` are distinct types with different semantics.

## Reference-level Explanation

### ArithType and ArithValue

```rust
// crates/kirin-arith/src/types.rs

/// Numeric types for arithmetic, mirroring Rust's primitive numeric types.
/// Bool is intentionally excluded — it belongs in the comparison dialect (kirin-cmp).
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, HasParser, PrettyPrint)]
pub enum ArithType {
    // Signed integers
    #[chumsky(format = "i8")]
    I8,
    #[chumsky(format = "i16")]
    I16,
    #[chumsky(format = "i32")]
    I32,
    #[chumsky(format = "i64")]
    I64,
    #[chumsky(format = "i128")]
    I128,
    // Unsigned integers
    #[chumsky(format = "u8")]
    U8,
    #[chumsky(format = "u16")]
    U16,
    #[chumsky(format = "u32")]
    U32,
    #[chumsky(format = "u64")]
    U64,
    #[chumsky(format = "u128")]
    U128,
    // Floats
    #[chumsky(format = "f32")]
    F32,
    #[chumsky(format = "f64")]
    F64,
}

impl Default for ArithType {
    fn default() -> Self {
        ArithType::I64
    }
}
```

`ArithValue` holds concrete compile-time values, mirroring `ArithType` one-to-one:

```rust
// crates/kirin-arith/src/types.rs

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum ArithValue {
    // Signed integers
    I8(i8),
    I16(i16),
    I32(i32),
    I64(i64),
    I128(i128),
    // Unsigned integers
    U8(u8),
    U16(u16),
    U32(u32),
    U64(u64),
    U128(u128),
    // Floats (stored as ordered-float for Hash/Eq)
    F32(OrderedFloat<f32>),
    F64(OrderedFloat<f64>),
}

impl Typeof<ArithType> for ArithValue {
    fn type_of(&self) -> ArithType {
        match self {
            ArithValue::I8(_) => ArithType::I8,
            ArithValue::I16(_) => ArithType::I16,
            ArithValue::I32(_) => ArithType::I32,
            ArithValue::I64(_) => ArithType::I64,
            ArithValue::I128(_) => ArithType::I128,
            ArithValue::U8(_) => ArithType::U8,
            ArithValue::U16(_) => ArithType::U16,
            ArithValue::U32(_) => ArithType::U32,
            ArithValue::U64(_) => ArithType::U64,
            ArithValue::U128(_) => ArithType::U128,
            ArithValue::F32(_) => ArithType::F32,
            ArithValue::F64(_) => ArithType::F64,
        }
    }
}
```

`ArithValue` uses a **manual `HasParser` implementation** (not derive) with heuristic parsing:
- Bare integer literal (e.g., `42`) → `I64(i64)` (default signed integer)
- Negative integer literal (e.g., `-5`) → `I64(i64)` (signed)
- Bare float literal (e.g., `3.14`) → `F64(f64)` (default float width)

The heuristic defaults to signed variants (I64 for integers, F64 for floats). Unsigned values are constructed programmatically (e.g., `ArithValue::U8(200)`). When used within a `Constant` statement, the surrounding type annotation provides the authoritative type. A future `{field:value}` format specifier (separate RFC) will allow format strings to control value-only printing and type-directed parsing.

### Dialect statement types

The dialect is a single flat enum with 6 operations. There is no separation between integer and float operations — the result type determines the semantics. Comparison operations are excluded (see `kirin-cmp` RFC).

```rust
// crates/kirin-arith/src/lib.rs

#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, HasParser, PrettyPrint)]
#[kirin(pure, fn, type = T)]
pub enum Arith<T: CompileTimeValue + Default> {
    #[chumsky(format = "{result:name} = add {lhs}, {rhs} -> {result:type}")]
    Add {
        lhs: SSAValue,
        rhs: SSAValue,
        result: ResultValue,
        #[kirin(default)]
        marker: std::marker::PhantomData<T>,
    },
    #[chumsky(format = "{result:name} = sub {lhs}, {rhs} -> {result:type}")]
    Sub {
        lhs: SSAValue,
        rhs: SSAValue,
        result: ResultValue,
        #[kirin(default)]
        marker: std::marker::PhantomData<T>,
    },
    #[chumsky(format = "{result:name} = mul {lhs}, {rhs} -> {result:type}")]
    Mul {
        lhs: SSAValue,
        rhs: SSAValue,
        result: ResultValue,
        #[kirin(default)]
        marker: std::marker::PhantomData<T>,
    },
    #[chumsky(format = "{result:name} = div {lhs}, {rhs} -> {result:type}")]
    Div {
        lhs: SSAValue,
        rhs: SSAValue,
        result: ResultValue,
        #[kirin(default)]
        marker: std::marker::PhantomData<T>,
    },
    #[chumsky(format = "{result:name} = rem {lhs}, {rhs} -> {result:type}")]
    Rem {
        lhs: SSAValue,
        rhs: SSAValue,
        result: ResultValue,
        #[kirin(default)]
        marker: std::marker::PhantomData<T>,
    },
    #[chumsky(format = "{result:name} = neg {operand} -> {result:type}")]
    Neg {
        operand: SSAValue,
        result: ResultValue,
        #[kirin(default)]
        marker: std::marker::PhantomData<T>,
    },
}
```

### Semantics and invariants

- All arithmetic operations are marked pure (`#[kirin(pure)]`) at the IR level.
- **Overflow behavior is abstract.** The IR does not specify whether integer overflow panics or wraps. A lowering pass determines the concrete behavior based on compilation mode (e.g., debug-mode panic checks vs release-mode wrapping), following Rust's model.
- **Division and remainder behavior is abstract.** The IR does not specify what happens on division by zero or signed overflow (`i32::MIN / -1`). A lowering pass determines whether to insert panic checks, trap, or leave as UB. A future `speculatable` attribute (see [Dependencies](#dependencies-on-other-rfcs)) will distinguish ops safe to speculatively execute.
- All operations take SSA values as inputs and produce a single `ResultValue`.
- The result type determines operation semantics. For example, `add %a, %b -> i32` is signed integer addition, `add %a, %b -> u32` is unsigned integer addition, and `add %a, %b -> f64` is float addition. Type checking (operand/result type consistency) is deferred to a verification pass.
- The type carries signedness: `i32` and `u32` are distinct types. This follows Rust convention rather than MLIR's signless integers.
- **Neg is restricted** to signed integer types (`I8`..`I128`) and float types (`F32`, `F64`), matching Rust's `Neg` trait. Using `neg` on unsigned types is a type error caught by verification.
- `ArithValue` and `ArithType` mirror each other one-to-one. `I32(42).type_of() == ArithType::I32`, `U32(42).type_of() == ArithType::U32`.
- `ArithValue` float variants use `ordered_float::OrderedFloat` to satisfy `Hash + Eq` requirements of `CompileTimeValue`.

### Crate impact matrix

| crate | impact | tests to update |
| --- | --- | --- |
| `kirin-arith` | New crate: dialect types, `ArithType`, `ArithValue`, parser/printer | new tests |
| `kirin-ir` | None (uses existing `Dialect`, `CompileTimeValue`, `Typeof` traits) | — |
| `kirin-chumsky` | None (derives handle parser generation) | — |
| `kirin-prettyless` | None (derives handle printer generation) | — |

### Module structure

```
crates/kirin-arith/
  Cargo.toml
  src/
    lib.rs          # Arith<T> enum (Add, Sub, Mul, Div, Rem, Neg), re-exports
    types.rs        # ArithType, ArithValue
```

### Dependencies

```toml
[dependencies]
kirin.workspace = true
ordered-float = "4"
```

## Drawbacks

- Adding a new dialect crate increases the dependency surface for downstream users — though this is opt-in.
- Explicit `ArithType` variants (I8..I128, U8..U128, F32, F64) cannot represent arbitrary widths. Users needing non-standard widths (e.g., i7, i256) must define their own type and use the generic `Arith<T>`.
- `ArithType` and `ArithValue` each have 12 variants (5 signed + 5 unsigned + 2 floats), which makes pattern matching verbose. This is the tradeoff for a 1:1 mapping with Rust's numeric primitives.
- `ArithValue`'s heuristic parser (bare `42` → I64, bare `3.14` → F64) means roundtrip fidelity for non-default widths (e.g., I8(42)) requires surrounding type context (e.g., a Constant statement). Standalone parse-print-parse of `ArithValue` is lossy for non-default widths.
- `ordered_float` is an external dependency required for `Hash + Eq` on float values.
- Overflow and division-by-zero behavior is abstract at the IR level, which means the IR alone doesn't tell you whether an operation is safe to speculate. Lowering passes must be aware of this.

## Rationale and Alternatives

### Proposed approach rationale

- Generic over `T` follows the established pattern from `ControlFlow<T>`, `Constant<T, Ty>`, and `StructuredControlFlow<T>` — no new abstraction needed.
- Unified operations (single `Add`, not separate `AddI`/`AddF`) because the generic type parameter already determines semantics. The opcode suffix is redundant when the result type is explicit. This also means the same `Arith<T>` dialect works across language boundaries — `Arith<ArithType>` for Rust semantics, `Arith<PythonNumType>` for Python semantics, etc. — without duplicating statement definitions.
- Signed types (`i32`, `u32` as distinct types) follow Rust convention, making the type system immediately familiar to Rust developers. This departs from MLIR's signless integers but is more natural for Kirin's Rust-native target audience.
- Providing `ArithType` and `ArithValue` as built-in types gives a complete, batteries-included experience while keeping the dialect itself generic.
- Explicit enum variants for `ArithType` (instead of `Int { width: u16 }`) make invalid states unrepresentable. Only standard hardware widths are supported.
- `ArithType` and `ArithValue` mirror each other 1:1 and both mirror Rust's primitive types. No ambiguity about what type a value has.
- Rust-style type names (`i32`, `f64`) are familiar to the target audience and shorter than MLIR-style (`int(32)`, `float(64)`).

### Alternative: Separate IntArith / FloatArith / Cmp sub-enums

- Description: Split into `IntArith<T>` (addi, subi, ...), `FloatArith<T>` (addf, subf, ...), and `Cmp<T>` (cmpi, cmpf) wrapped by a top-level `Arith<T>` enum via `#[wraps]`.
- Pros: Matches MLIR convention. Easy to pattern-match on just integer or just float ops. Each sub-enum is small.
- Cons: Redundant — the result type already encodes int vs float. Doubles the number of operation variants for the same semantics. Doesn't leverage the generic type parameter.
- Reason not chosen: With a generic `T`, the type system determines semantics. Separating by int/float at the opcode level is unnecessary duplication. A single flat enum is simpler and more aligned with the generic design.

### Alternative: Signless integer types (MLIR convention)

- Description: A single `I32` type for both signed and unsigned 32-bit integers. The operation determines signed/unsigned semantics (e.g., `sdiv` vs `udiv`).
- Pros: Fewer type variants. Matches MLIR convention.
- Cons: Unfamiliar to Rust developers. Requires separate signed/unsigned opcodes for div/rem. Values lose sign information or need a separate signedness field.
- Reason not chosen: Since `ArithValue` already carries both signed and unsigned Rust types, having signed types is the consistent completion of that design. Rust-native developers expect `i32` and `u32` to be distinct types.

### Alternative: No built-in ArithType — only generic dialect

- Description: Only provide `Arith<T>` with no concrete `ArithType`.
- Pros: Maximum flexibility, no opinions about types.
- Cons: Every user must define their own numeric type system from scratch, losing the "batteries included" value.
- Reason not chosen: Providing `ArithType` as an optional, standalone type covers the 80% case without limiting the generic dialect.

### Alternative: Parameterized ArithType with width field

- Description: `ArithType::Int { width: u16 }` / `ArithType::Float { width: u16 }` with runtime validation.
- Pros: Supports arbitrary widths (e.g., `int(7)`, `int(256)`). Fewer enum variants.
- Cons: Invalid states are representable. Requires runtime validation in constructors or parsers. Harder to match exhaustively.
- Reason not chosen: Explicit variants make invalid states unrepresentable. Standard hardware widths cover the practical use cases. Users needing arbitrary widths use the generic `Arith<T>` with their own type.

### Alternative: Single Int(i128) variant for ArithValue

- Description: `ArithValue::Int(i128)` for all integer widths, with `type_of()` always returning I64.
- Pros: Fewer variants, simpler enum.
- Cons: `type_of()` is inaccurate for non-I64 values. Width and sign interpretation are lost. Requires external tracking to know the actual type of a value.
- Reason not chosen: Per-width signed/unsigned variants (I8(i8)/U8(u8), ...) ensure `type_of()` is always correct and sign intent is preserved.

### Alternative: Generic byte-storage value type

- Description: `IntValue { bits: u128, width: u16 }` — store all integers as bit patterns with explicit width.
- Pros: Single variant, width-aware, supports up to 128 bits uniformly.
- Cons: Requires manual bit-width-aware arithmetic for constant folding. Less ergonomic than native Rust types. Pattern matching doesn't give you the concrete type.
- Reason not chosen: Keeping things simple with native Rust types per variant. Users needing more sophisticated value representations can define their own via the generic parameter.

## Prior Art

- [MLIR `arith` dialect](https://mlir.llvm.org/docs/Dialects/ArithOps/): The primary inspiration for operation structure. Kirin's `kirin-arith` departs from MLIR's `arith` in several fundamental ways, all stemming from one key difference: **Kirin's statements are generic over the type parameter (`Arith<T>`)**, while MLIR's operations are monomorphic with a fixed type system.

  This has cascading design consequences:

  | | MLIR `arith` | Kirin `kirin-arith` |
  | --- | --- | --- |
  | **Type system** | Fixed: signless integers (`i32`), floats (`f32`) | Generic: `T: CompileTimeValue + Default`. `ArithType` is one option. |
  | **Signedness** | Signless — the operation encodes sign (`sdiv` vs `udiv`) | Types carry signedness (`i32` vs `u32`). One `div` op. |
  | **Int/float split** | Separate opcodes: `addi`, `addf`, `muli`, `mulf` | Unified: single `add`, `mul`. Type determines semantics. |
  | **Comparisons** | In arith dialect: `cmpi`, `cmpf` with MLIR predicates | Separate `kirin-cmp` dialect with Rust-style `eq`/`lt`/`ge`. |
  | **Cross-language reuse** | Must redefine ops for different type systems | Same `Arith<T>` works for Rust, Python, Julia, etc. by swapping `T`. |
  | **Overflow/div-by-zero** | Defined per-operation (UB, trap, etc.) | Abstract at IR level — lowering pass decides. |
  | **Negation** | `arith.negf` (float only; int neg via `0 - x`) | `neg` for signed integers + floats. Unsigned rejected by verification. |

  The generic design means MLIR would need separate `python_arith.add` and `rust_arith.add` dialect ops for different languages, while Kirin uses a single `Arith<T>::Add` parameterized over the type. This is the fundamental advantage of Rust's type-level generics applied to IR design.

- [MLIR `math` dialect](https://mlir.llvm.org/docs/Dialects/MathOps/): Transcendental functions are intentionally excluded from this RFC, mirroring MLIR's separation of `arith` (basic ops) from `math` (transcendentals).
- [Cranelift IR](https://github.com/bytecodealliance/wasmtime/tree/main/cranelift): Uses typed opcodes (`iadd`, `fadd`) with explicit types on values. Like MLIR, Cranelift's operations are monomorphic — each opcode is defined once for a fixed type system. Kirin's generic approach avoids this duplication.
- Existing Kirin dialects (`kirin-cf`, `kirin-scf`, `kirin-constant`): Establish the patterns for generic parameterization, derive macros, and text format conventions that this RFC follows.

## Backward Compatibility and Migration

- Breaking changes: None. This is a new crate with no existing users.
- Migration steps: Not applicable.
- Compatibility strategy: The crate is additive. Existing languages that define their own arithmetic can adopt `kirin-arith` incrementally by replacing custom dialect types with `Arith<T>`.

## How to Teach This

- Document `ArithType` as the go-to type system for languages that need standard numeric types.
- Show the composition pattern in a "getting started" example: `Arith<ArithType>` + `Constant<ArithValue, ArithType>` + `ControlFlow<ArithType>` as a minimal numeric language.
- Explain that `ArithType` mirrors Rust's primitive types — `i32` and `u32` are distinct types with different semantics, just like in Rust.
- Explain that operations are type-generic: `add %a, %b -> i32` is signed integer addition, `add %a, %b -> u32` is unsigned, `add %a, %b -> f64` is float. No `addi`/`addf` distinction needed.
- Explain that `neg` only works on signed integers and floats, matching Rust's `Neg` trait.
- Explain that overflow and division-by-zero behavior is determined by the lowering pass, not the IR, following Rust's debug/release mode distinction.
- Emphasize that `ArithType`/`ArithValue` are convenience types — users with custom type systems use `Arith<MyType>` with their own types via the generic parameter.

## Reference Implementation Plan

1. Define `ArithType` (with `HasParser`/`PrettyPrint` derives) and `ArithValue` (with manual `HasParser`, `PrettyPrint` derive) in `crates/kirin-arith/src/types.rs`.
2. Implement `Arith<T>` (Add, Sub, Mul, Div, Rem, Neg) in `crates/kirin-arith/src/lib.rs` with derive macros.
3. Add parser roundtrip tests for all operations and types.
4. Add an integration test composing `Arith<ArithType>` into a full language with constants and control flow.

### Acceptance Criteria

- [ ] All arithmetic operations (add, sub, mul, div, rem, neg) parse and print correctly with both integer and float types.
- [ ] `neg` is restricted to signed integer and float types.
- [ ] `ArithType` parses and prints Rust-style type names (`i8`..`i128`, `u8`..`u128`, `f32`, `f64`).
- [ ] `ArithValue` implements `Typeof<ArithType>` with accurate per-variant type mapping.
- [ ] `ArithValue` heuristic parser handles integer literals and float literals.
- [ ] `Arith<ArithType>` composes into a multi-dialect language with `Constant` and `ControlFlow`.
- [ ] `print -> parse -> print` roundtrip holds for all operation formats.
- [ ] All operations are marked pure via `#[kirin(pure)]`.

### Tracking Plan

- Tracking issue: TBD
- Implementation PRs: TBD

### Dependencies on other RFCs

This RFC has the following related/dependent RFCs:

**`kirin-cmp` RFC** (separate, not blocking):
- Comparison operations (`Cmp` with `Eq`/`Ne`/`Lt`/`Le`/`Gt`/`Ge` predicates), `IsNan` unary op, and `BoolType`/`BoolValue`.
- Mirrors Rust's separation of `PartialEq`/`PartialOrd` from arithmetic traits.
- Languages needing both arithmetic and comparisons compose both dialects.

**Speculatable attribute RFC** (future):
- Define `#[kirin(speculatable)]` as an independent flag orthogonal to `#[kirin(pure)]`.
- `pure` = no side effects; `speculatable` = safe to execute speculatively (no traps).
- Most arithmetic ops will be both. Div/rem will be pure but not speculatable.

**`{field:value}` format specifier RFC** (future, not blocking):
- New chumsky format specifier for value-only printing/parsing.
- Not needed for this RFC — `ArithValue` uses a manual parser.

## Unresolved Questions

- Should this RFC describe `kirin-arith` as populating an existing workspace crate (already present in `Cargo.toml`) instead of introducing a new crate from scratch?
- What is the intended dependency and landing order across related RFCs (`0003` speculatable, `0004` value format specifier, and future `kirin-cmp`)?
- Which acceptance criteria in this RFC are explicitly blocked by other RFCs versus implementable independently?

## Future Possibilities

- `kirin-cmp` dialect: comparison operations, `BoolType`, `IsNan`, and future logical operations (`And`, `Or`, `Not`, `Xor` on bools).
- `#[kirin(speculatable)]` attribute for distinguishing trapping from non-trapping pure ops.
- `{field:value}` format specifier for type-directed value parsing/printing in chumsky format strings.
- Checked arithmetic operations (`checked_add`, `checked_mul`, etc.) that return `Option`-like results.
- Wrapping/saturating arithmetic operations (`wrapping_add`, `saturating_mul`, etc.) matching Rust's explicit APIs.
- Bitwise and shift operations in a separate `kirin-bitwise` dialect.
- Type cast operations (`as`-style casts between numeric types) in a `kirin-cast` dialect.
- Transcendental math operations in a `kirin-math` dialect.
- SIMD/vector support for element-wise arithmetic.
- Constant folding pass that evaluates `ArithValue` operations at compile time.
- Builtin numeric types in `kirin-ir` with `Typeof` impls for Rust primitives (if orphan rule can be resolved).

## Revision Log

| Date | Change |
| --- | --- |
| 2026-02-09T01:38:49.788023Z | RFC created from template |
| 2026-02-09 | Major revision: ArithType mirrors Rust numeric primitives (I8..I128, U8..U128, F32, F64); ArithValue mirrors 1:1; unified int/float ops; Cmp/Bool extracted to separate kirin-cmp RFC; Neg restricted to signed+float; overflow/div behavior abstract (deferred to lowering) |
