# Dialects -- Implementer (Code Quality) Review

**Crates:** kirin-cf, kirin-scf, kirin-constant, kirin-arith, kirin-bitwise, kirin-cmp, kirin-function
**Total:** ~3295 lines

## Clippy Audit

| File | Allow | Justified? |
|------|-------|------------|
| `kirin-arith/src/types/arith_type.rs:42` | `clippy::derivable_impls` | Yes -- `Default` returns `I64` (not first variant), making intent explicit. Flagged in Phase 1. |

No other `#[allow]` in any dialect crate.

## Findings

### D1. Binary-op interpret boilerplate across Arith/Bitwise/Cmp (P2, high confidence)

All three crates repeat the same pattern: read lhs, read rhs, apply op, write result, return `Ok(Continuation::Continue)`. This appears 6 times in `kirin-arith/src/interpret_impl.rs:37-91`, 6 times in `kirin-bitwise/src/interpret_impl.rs:26-73`, and 6 times in `kirin-cmp/src/interpret_impl.rs:235-284`. A helper like `interp.binary_op(lhs, rhs, result, |a, b| a + b)` on `Interpreter` (or a free function) would eliminate ~100 lines across these three crates. Note: this is related to Phase 1 finding P1-8 (derive Interpretable for inner enums).

### D2. `_ => unreachable!()` in `#[non_exhaustive]` enum match arms (P3, high confidence)

`kirin-arith/src/interpret_impl.rs:91`, `kirin-bitwise/src/interpret_impl.rs:74`, `kirin-cmp/src/interpret_impl.rs:284`, `kirin-cf/src/interpret_impl.rs:61` all have `_ => unreachable!()` to handle the `__Phantom` variant. This is correct but fragile -- if a new variant is added, the catch-all silently absorbs it. Consider matching `Self::__Phantom(..) => unreachable!()` explicitly so new variants trigger a compiler error.

### D3. FunctionBody/Lambda SSACFGRegion + Interpretable duplication (P2, high confidence)

`kirin-function/src/interpret_impl.rs:9-43` and `:45-79` are nearly identical for `FunctionBody` and `Lambda` -- both implement `SSACFGRegion::entry_block` and `Interpretable::interpret` with identical logic (get first block from region, return Jump). A shared helper or blanket impl over a `HasRegionBody` trait would eliminate ~35 lines of duplication.

### D4. `Interval` fields are `pub` (P3, medium confidence)

`kirin-interval/src/interval/domain.rs:5-8`: `lo` and `hi` are public fields on `Interval`. This allows constructing invalid intervals (e.g., `Interval { lo: Finite(5), hi: Finite(3) }`) bypassing the `new()` constructor that normalizes to bottom. Consider private fields with accessors, similar to Phase 1 finding on `Signature` fields (P3-3).

**Note:** This finding is in the utilities group but relates to dialect-level semantics.

## Summary

- 1 `#[allow]` found, justified
- Main concern is interpret boilerplate (~100 lines across 3 crates)
- Dialect type definitions are clean and well-structured; the derive-heavy approach works well
- Feature gating (`#[cfg(feature = "interpret")]`) is consistently applied
