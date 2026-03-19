# Utilities -- Physicist (Ergonomics/DX) Review

**Crates:** kirin-lexer, kirin-interval
**Lines:** ~2445

## Scenario: "I want to add an interval analysis for my type"

The interval crate provides `Interval` with `Lattice`, `BranchCondition`, `AbstractValue` (widen/narrow), and arithmetic operations via `std::ops` impls. To add interval analysis for a new type, you would: (1) implement `From<YourValue> for Interval`, (2) use `AbstractInterpreter` with `Interval` as the value domain. The crate is self-contained and well-structured.

## Concept Budget: Interval Analysis

| Concept | Required? | Where learned |
|---------|-----------|---------------|
| `Interval` (domain type) | Yes | kirin-interval |
| `Bound` (NegInf/Finite/PosInf) | Yes | kirin-interval |
| `Lattice` trait (join/meet/is_subseteq) | Yes | kirin-ir |
| `AbstractValue` (widen/narrow) | Yes | kirin-interpreter |
| `BranchCondition` (is_truthy) | Yes | kirin-interpreter |
| `AbstractInterpreter` setup | Yes | kirin-interpreter |
| Arithmetic impls (Add/Sub/Mul/Neg) | If needed | kirin-interval |

**Total: 5-7 concepts.** Reasonable for abstract interpretation, which is inherently complex.

## Findings

### U1. kirin-lexer `ToTokens` impl is ~100 lines of mechanical match arms (P3, low confidence)

`kirin-lexer/src/lib.rs:144-244`: The `ToTokens` impl behind `#[cfg(feature = "quote")]` repeats the same `tokens.extend(quote::quote! { Token::Variant })` pattern for every variant. A macro could reduce this, but it is gated behind an optional feature and is maintenance rather than DX. Low priority.

### U2. Interval arithmetic functions are free-standing, not method-based (P3, medium confidence)

The public API exports `interval_add`, `interval_mul`, `interval_neg`, `interval_sub` as free functions (`kirin-interval/src/lib.rs:4`). However, `Interval` also implements `std::ops::Add`, `Sub`, `Mul`, `Neg` (via `arith_impl.rs`). The free functions and operator impls coexist. A user might be confused about which to use. The `std::ops` impls are more idiomatic for Rust users; the free functions are useful for non-consuming contexts. Consider documenting this duality.

### U3. `Interval::bottom_interval()` naming is unusual (P3, medium confidence)

`kirin-interval/src/interval/domain.rs:26-31`: The constructor is named `bottom_interval()` rather than just `bottom()`. The `HasBottom` trait already provides `Interval::bottom()` via `lattice_impl.rs`. The explicit `bottom_interval()` is likely needed because `HasBottom::bottom()` requires the trait in scope. Consider adding a doc alias or note pointing users to `HasBottom::bottom()`.

### U4. kirin-lexer is clean and self-contained (strength)

Single file, 91 lines of token definitions plus Display impl. The `Logos` derive handles all lexing. No ergonomic issues for users -- the Token enum is imported through `kirin::parsers::Token` and users rarely interact with the lexer directly.

### U5. Interval crate feature-gates dialect-specific impls well (strength)

Arithmetic (`feature = "arith"`), comparison (`feature = "cmp"`), and interpreter (`feature = "interpreter"`) impls are all behind features. A user who only needs the interval domain for their own analysis can depend on just the core without pulling in kirin-arith or kirin-cmp. Good modularity.
