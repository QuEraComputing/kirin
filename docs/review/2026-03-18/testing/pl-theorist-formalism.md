# Testing -- PL Theorist Formalism Review

## Findings

### T1. Lattice law checker is thorough and correctly structured (positive, confirmed)

`kirin-test-utils/src/lattice.rs:220-347` checks all five standard lattice laws (join/meet commutativity, associativity, idempotence, absorption, ordering consistency) plus bottom/top identity and annihilation. The violation-collection pattern (lines 23-32) reports all failures at once rather than short-circuiting. This matches the standard algebraic characterization of bounded lattices from Birkhoff's lattice theory. The checker also correctly tests `is_subseteq` consistency with both `join` and `meet` (lines 287-306), which is the defining equivalence for lattice order.

**File:** `crates/kirin-test-utils/src/lattice.rs:51-216`

### T2. Lattice checker does not verify distributivity (P3, confirmed)

The checker tests lattice laws but not *distributive* lattice laws (`a.join(&b.meet(&c)) == a.join(&b).meet(&a.join(&c))`). Some Kirin lattices (e.g., `SimpleType` with flat structure) happen to be distributive, but the checker does not verify this. For abstract interpretation, distributivity is not required (Cousot-style frameworks use general lattices), so this is not a correctness concern. An `assert_distributive_laws` helper would be useful if any analysis relies on distributivity.

**File:** `crates/kirin-test-utils/src/lattice.rs:138-145`

### T3. `SimpleType` Default returns `bottom()` -- aligns with Phase 1 finding (P3, confirmed)

`kirin-test-types/src/simple_type.rs:55-59`: `Default for SimpleType` returns `Self::bottom()`. Phase 1 finding P2-D flagged `TypeLattice`'s `Default` requirement as problematic since `Default` has no formal relationship to `bottom()`. Here we see a test type that happens to equate them, reinforcing that the `Default` bound should be removed in favor of explicit `Placeholder` usage (already accepted in Phase 1).

**File:** `crates/kirin-test-types/src/simple_type.rs:55-59`

### T4. `UnitType` is a trivial one-element lattice -- limited test coverage (P3, confirmed)

`kirin-test-types/src/unit_type.rs:13-39`: `UnitType` is a degenerate lattice where `top == bottom`. It satisfies all lattice laws vacuously. The lattice test suite at `lattice.rs:354-357` only tests `UnitType`, which cannot catch bugs in join/meet/ordering logic since all operations are identity. `SimpleType` (7+ elements with non-trivial ordering) would provide stronger validation.

**File:** `crates/kirin-test-utils/src/lattice.rs:354-357`

### T5. `Value` lacks `Eq` -- manual `Hash` without `Eq` (P3, confirmed)

`kirin-test-types/src/value.rs:4-8`: `Value` derives `PartialEq` and manually implements `Hash` (using `f64::to_bits`), but does not implement `Eq`. The `Hash` contract requires that `a == b` implies `hash(a) == hash(b)`, which holds since `PartialEq` derives structural equality. However, `f64`'s `NaN != NaN` means `Value::F64(NaN) != Value::F64(NaN)` while `hash` would be equal -- this is a standard `f64` hazard, acceptable for test types.

**File:** `crates/kirin-test-types/src/value.rs:4-8`

## Summary

The lattice testing infrastructure is well-designed and aligns with standard algebraic definitions. The main gap is that the only lattice actually tested in the suite is the trivial `UnitType`. Adding `SimpleType` and `Interval` to the lattice law tests would strengthen coverage significantly.
