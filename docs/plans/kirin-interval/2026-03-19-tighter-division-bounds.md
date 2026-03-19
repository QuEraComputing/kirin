# Tighter Interval Division and Remainder Bounds

## Problem

The current `Div` and `Rem` implementations for `Interval` return `Interval::top()` unconditionally:

```rust
impl std::ops::Div for Interval {
    type Output = Self;
    fn div(self, _rhs: Self) -> Self {
        Interval::top()
    }
}

impl std::ops::Rem for Interval {
    type Output = Self;
    fn rem(self, _rhs: Self) -> Self {
        Interval::top()
    }
}
```

This is sound but maximally imprecise. For abstract interpretation, tighter bounds significantly improve analysis precision, particularly in loop analyses where division and modular arithmetic are common.

## Research Findings

### Current arithmetic infrastructure

`crates/kirin-interval/src/interval/arithmetic.rs` provides:
- `interval_add(a, b)` -- endpoint addition with saturation
- `interval_sub(a, b)` -- endpoint subtraction with saturation
- `interval_mul(a, b)` -- 4-corner product formula (min/max of all endpoint products)
- `interval_neg(a)` -- negation via bound swap

All correctly handle empty intervals (return bottom). The `Bound` type supports `NegInf`, `PosInf`, and `Finite(i64)` with saturating arithmetic.

### Bound operations available

From `bound.rs` (inferred from usage): `saturating_add`, `saturating_sub`, `saturating_mul`, `negate`, `min`, `max`, `less_eq`, `less_than`.

Division and remainder on `Bound` are not currently implemented -- they would need to be added.

### Interval domain structure

```rust
pub struct Interval {
    pub lo: Bound,
    pub hi: Bound,
}
```

An interval is empty when `lo > hi` (via `is_empty()`). The bottom element is `bottom_interval()` (empty). The top element is `[NegInf, PosInf]`.

### Comparison impl for reference

`crates/kirin-interval/src/interval/cmp_impl.rs` shows the pattern for handling interval operations: check for empty, check for definite cases, fall back to over-approximation.

## Proposed Design

### Division: `interval_div(a, b) -> Interval`

**Case analysis on the divisor `b`:**

1. **Either operand empty:** return bottom.
2. **Divisor spans zero** (`b.lo <= 0 <= b.hi`): return `top()`. Division by zero is undefined; any value is possible. (A refinement splitting `b` into negative/positive halves is possible but complex -- defer.)
3. **Divisor is strictly positive** (`b.lo > 0`): Use 4-corner formula.
   - `a.lo / b.lo`, `a.lo / b.hi`, `a.hi / b.lo`, `a.hi / b.hi`
   - Take min for `lo`, max for `hi`.
   - Integer division truncates toward zero, so the 4-corner formula is sound but may slightly over-approximate (which is fine for an upper bound).
4. **Divisor is strictly negative** (`b.hi < 0`): Negate both divisor and dividend, then apply case 3. `a / b == (-a) / (-b)` for integer division.

**Bound-level division:**
- `Finite(a) / Finite(b)` = `Finite(a / b)` (Rust integer division truncates toward zero)
- `PosInf / Finite(b)` where `b > 0` = `PosInf`
- `NegInf / Finite(b)` where `b > 0` = `NegInf`
- `PosInf / Finite(b)` where `b < 0` = `NegInf`
- `NegInf / Finite(b)` where `b < 0` = `PosInf`
- `Inf / Inf` cases should not arise if divisor does not span zero.

**Subtlety with integer truncation:** For negative dividends with positive divisors, `(-7) / 2 = -3` (truncates toward zero). The 4-corner formula handles this correctly because we take the min/max of all corners.

### Remainder: `interval_rem(a, b) -> Interval`

**Key property of integer remainder:** `a % b` has the same sign as `a` (in Rust), and `|a % b| < |b|`.

**Case analysis:**

1. **Either operand empty:** return bottom.
2. **Divisor spans zero:** return `top()`.
3. **Divisor does not span zero:** Let `M = max(|b.lo|, |b.hi|) - 1`. Then:
   - If `a.lo >= 0`: result is in `[0, min(a.hi, M)]`
   - If `a.hi <= 0`: result is in `[max(a.lo, -M), 0]`
   - If `a` spans zero: result is in `[max(a.lo, -M), min(a.hi, M)]`

This is significantly tighter than `top()` in most cases. For example, `[0, 100] % [3, 3]` yields `[0, 2]` instead of `[-inf, +inf]`.

### Implementation structure

Add two functions in `arithmetic.rs`:
```rust
pub fn interval_div(a: &Interval, b: &Interval) -> Interval { ... }
pub fn interval_rem(a: &Interval, b: &Interval) -> Interval { ... }
```

Add `Bound::saturating_div` in `bound.rs` for the division cases.

Update `lattice_impl.rs` to delegate:
```rust
impl std::ops::Div for Interval {
    type Output = Self;
    fn div(self, rhs: Self) -> Self { interval_div(&self, &rhs) }
}

impl std::ops::Rem for Interval {
    type Output = Self;
    fn rem(self, rhs: Self) -> Self { interval_rem(&self, &rhs) }
}
```

## Implementation Steps

1. Add `Bound::saturating_div(self, other: Bound) -> Bound` in `bound.rs`, handling the finite and infinite cases.
2. Add `interval_div` in `arithmetic.rs` with the case analysis described above.
3. Add `interval_rem` in `arithmetic.rs` with the remainder bounds.
4. Update `Div` and `Rem` impls in `lattice_impl.rs` to delegate.
5. Export the new functions from `mod.rs` alongside the existing exports.
6. Add comprehensive tests.

## Risk Assessment

**Medium risk.** The correctness of interval division with integer truncation semantics requires careful handling of signs. The 4-corner approach is sound for division when the divisor does not span zero, but edge cases with `Bound::PosInf` and `Bound::NegInf` need attention.

Key risk areas:
- **Division by zero spanning:** The `top()` fallback is conservative and safe. Future refinement (splitting divisor into positive/negative halves) can be done later.
- **Integer truncation direction:** Rust truncates toward zero. This differs from floor division. The 4-corner formula still over-approximates soundly, but the bounds may not be as tight as possible. Tighter bounds for truncation division are complex and can be deferred.
- **Overflow:** Saturating arithmetic on bounds should prevent overflow, matching the existing multiplication implementation.

## Testing Strategy

- **Unit tests for `interval_div`:**
  - `[6, 12] / [2, 3]` should yield `[2, 6]`
  - `[-12, -6] / [2, 3]` should yield `[-6, -2]`
  - `[-6, 6] / [2, 3]` should yield `[-3, 3]`
  - `[a, b] / [c, d]` where `c <= 0 <= d` should yield `top()`
  - Empty interval inputs should yield bottom
  - `[5, 5] / [2, 2]` should yield `[2, 2]`
  - `[-7, -7] / [2, 2]` should yield `[-3, -3]` (truncation toward zero)

- **Unit tests for `interval_rem`:**
  - `[0, 100] % [3, 3]` should yield `[0, 2]`
  - `[-100, 0] % [3, 3]` should yield `[-2, 0]`
  - `[-50, 50] % [7, 7]` should yield `[-6, 6]`
  - `[0, 2] % [10, 10]` should yield `[0, 2]` (tighter: min of `a.hi` and `M`)
  - Empty and zero-spanning divisor cases

- **Property tests:** Verify soundness by checking that for random concrete values `a in interval_a` and `b in interval_b`, `a / b` is contained in `interval_div(interval_a, interval_b)`.

- **Regression tests:** Ensure existing `arith_tests` and `branch_tests` still pass.
