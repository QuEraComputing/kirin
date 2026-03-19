# Separate CompareValue Result Domain from Operand Domain

## Problem

`CompareValue` in `crates/kirin-cmp/src/interpret_impl.rs:10-17` returns `Self`:

```rust
pub trait CompareValue {
    fn cmp_eq(&self, other: &Self) -> Self;
    fn cmp_ne(&self, other: &Self) -> Self;
    fn cmp_lt(&self, other: &Self) -> Self;
    fn cmp_le(&self, other: &Self) -> Self;
    fn cmp_gt(&self, other: &Self) -> Self;
    fn cmp_ge(&self, other: &Self) -> Self;
}
```

The result type is the same as the operand type. This works for `i64` (returning 0/1) but is semantically wrong for abstract domains. The `Interval` implementation returns `Interval::new(0, 1)` or `Interval::constant(0/1)` -- an interval over booleans encoded as integers. This conflates the integer domain with the boolean domain.

In a more precise abstract interpretation, the comparison result should be a boolean abstract value (e.g., `{true}`, `{false}`, `{true, false}`) rather than an interval `[0, 1]`. The interval `[0, 1]` is an over-approximation that loses the boolean structure -- for example, it does not express "definitely true" vs "definitely false" with the same precision as a dedicated boolean domain.

## Research Findings

### Current `CompareValue` implementors

1. **`i64`** (`crates/kirin-cmp/src/interpret_impl.rs:19-38`): Returns `0` or `1`. Works correctly because booleans are encoded as integers in the concrete domain.

2. **`Interval`** (`crates/kirin-interval/src/interval/cmp_impl.rs:3-85`): Returns `Interval::constant(0)`, `Interval::constant(1)`, or `Interval::new(0, 1)`. This works but is imprecise: an `Interval` result `[0, 1]` cannot distinguish between "might be true or false" and "is an integer between 0 and 1" in downstream arithmetic.

### How `CompareValue` is used

In `interpret_impl.rs:223-286`, the `Cmp<T>` dialect's `Interpretable` impl calls `interp.read(*lhs)?.cmp_eq(&interp.read(*rhs)?)` and writes the result back. The interpreter's `Value` type must implement `CompareValue`, and the result is stored as the same `Value` type.

The `I::Value: CompareValue` bound in the `Interpretable` impl means the comparison result must be the same type as interpreter values. This is the root constraint.

### Impact on `BranchCondition`

The `kirin-interpreter` framework uses `BranchCondition` to evaluate branch conditions. If `I::Value` is `Interval`, then branch conditions operate on intervals. The comparison result `[0, 1]` is used in `BranchCondition::is_true/is_false` checks. The interval domain's `BranchCondition` impl likely checks whether the interval contains 0 or non-zero values.

A separate boolean domain would need its own `BranchCondition` impl, or the conversion from `Bool` back to `Value` would need to happen at the interpreter level.

## Proposed Design

### Add `Bool` associated type to `CompareValue`

```rust
pub trait CompareValue {
    type Bool;

    fn cmp_eq(&self, other: &Self) -> Self::Bool;
    fn cmp_ne(&self, other: &Self) -> Self::Bool;
    fn cmp_lt(&self, other: &Self) -> Self::Bool;
    fn cmp_le(&self, other: &Self) -> Self::Bool;
    fn cmp_gt(&self, other: &Self) -> Self::Bool;
    fn cmp_ge(&self, other: &Self) -> Self::Bool;
}
```

### Implementor updates

**`i64`:** `type Bool = i64;` -- no behavior change. Booleans are integers.

**`Interval`:** Two options:

**Option A (minimal):** `type Bool = Interval;` -- no behavior change. The type alias documents intent while allowing future refinement.

**Option B (full):** `type Bool = BoolInterval;` where `BoolInterval` is a 3-element domain: `{ True, False, Unknown }`. This is isomorphic to `Interval::new(1,1)`, `Interval::new(0,0)`, `Interval::new(0,1)` but more principled.

### Interpreter integration

The `Cmp<T>` interpretable impl writes the comparison result via `interp.write(*result, value)`. Currently `value: I::Value`. With the associated type, `value: <I::Value as CompareValue>::Bool`.

For this to work, `Bool` must be convertible to `I::Value` for the write. Two approaches:

**Approach 1: `Bool = Value` (zero-cost migration).** Keep `type Bool = Self;` as default. Implementors opt in to a separate domain later. The trait change is non-breaking.

**Approach 2: `Bool: Into<Value>` bound.** Add `where Self::Bool: Into<Self>` or similar. The interpreter's write path converts `Bool` to `Value`. This is cleaner but requires more changes.

### Recommendation: Phased approach

**Phase 1:** Add `type Bool = Self;` with a default:
```rust
pub trait CompareValue {
    type Bool = Self;
    fn cmp_eq(&self, other: &Self) -> Self::Bool;
    // ...
}
```

Wait -- Rust does not support associated type defaults on stable. Alternative:

**Phase 1 (actual):** Add `type Bool;` without default. Update all implementors to `type Bool = Self;`. Update the `Interpretable` impl to use `<I::Value as CompareValue>::Bool` and add `Into<I::Value>` bound on `Bool`. This is a breaking change but contained to 2 implementors and 1 interpreter impl.

**Phase 2:** Introduce `BoolInterval` in `kirin-interval` and switch `Interval`'s `Bool` to it. Add `From<BoolInterval> for Interval` conversion.

## Implementation Steps

### Phase 1

1. Add `type Bool;` to `CompareValue` trait definition.
2. Update `impl CompareValue for i64`: add `type Bool = i64;`.
3. Update `impl CompareValue for Interval`: add `type Bool = Interval;`.
4. Update `Cmp<T>`'s `Interpretable` impl to use the `Bool` associated type:
   ```rust
   I::Value: CompareValue,
   <I::Value as CompareValue>::Bool: Into<I::Value>,  // or just keep the same type
   ```
5. Update `interp.write(*result, a.cmp_eq(&b))` -- if `Bool = Value`, no change needed. If `Bool != Value`, add `.into()`.
6. Verify all tests pass.

### Phase 2 (implement together with Phase 1)

1. Define `BoolInterval` enum in `kirin-interval`: `True`, `False`, `Unknown`, `Bottom`.
2. Implement `From<BoolInterval> for Interval`.
3. Change `Interval`'s `type Bool = BoolInterval;`.
4. Update `BranchCondition` for `BoolInterval` if needed.

## Risk Assessment

**Phase 1: Low risk.** With `type Bool = Self` on all implementors, the only change is adding the associated type. The interpreter impl needs a minor bound update. No behavioral change.

**Phase 2: Medium risk.** Introducing `BoolInterval` requires careful integration with `BranchCondition` and any code that operates on comparison results. The `Into<Interval>` conversion must be correct (`True -> [1,1]`, `False -> [0,0]`, `Unknown -> [0,1]`, `Bottom -> bottom`).

Key risk: downstream code that pattern-matches on `I::Value` after a comparison may break if `Bool` is a different type. This is actually a benefit -- it forces explicit handling of the boolean domain.

## Testing Strategy

- All existing `CompareValue` tests for `i64` pass unchanged (Phase 1).
- All existing `CompareValue` tests for `Interval` pass unchanged (Phase 1).
- All `kirin-cmp` interpretation tests pass unchanged.
- Phase 2: Add `BoolInterval` lattice tests (join, meet, is_subseteq, bottom, top).
- Phase 2: Add conversion tests `BoolInterval -> Interval` and back.
- Phase 2: Add interpreter integration tests verifying comparison results flow correctly through branch conditions.
