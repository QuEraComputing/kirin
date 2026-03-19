# Remove `Default` from `TypeLattice`

## Problem

`TypeLattice` is defined as:
```rust
pub trait TypeLattice: FiniteLattice + CompileTimeValue + Default {}
```

The `Default` bound is semantically ambiguous: is `Default::default()` intended to be `bottom()`, `top()`, or something else? The review finding (P2-D) notes this creates a risk of subtle bugs in dispatch logic (`LatticeSemantics` uses `TypeLattice`). The relationship between `Default::default()` and `HasBottom::bottom()` is unspecified.

The user directive is clear: remove `Default`, report callers.

## Research Findings

### `TypeLattice` definition

Located at `crates/kirin-ir/src/lattice.rs:59`:
```rust
pub trait TypeLattice: FiniteLattice + CompileTimeValue + Default {}
```

### Callers and implementors

**Trait usage sites (bound `T: TypeLattice`):**

1. `crates/kirin-ir/src/signature/semantics.rs:88,90` -- `LatticeSemantics<T: TypeLattice>` struct and impl. Uses `is_subseteq` only; does NOT call `default()`.

2. `crates/kirin-chumsky/src/parsers/blocks.rs` -- comments reference "TypeLattice" but only use `HasParser<'t>` bounds, not `TypeLattice` directly.

3. `crates/kirin-chumsky/src/parsers/function_type.rs:10` -- same, comments only.

4. `crates/kirin-chumsky/src/parsers/values.rs:26,49,79` -- same, comments only.

5. `crates/kirin-chumsky/src/function_text/tests.rs:6,54` -- test types implement `TypeLattice`.

**Implementors of `TypeLattice`:**

1. `crates/kirin-test-types/src/simple_type.rs:53` -- `impl TypeLattice for SimpleType {}` with `Default` returning `Self::bottom()` (line 55-58).

2. `crates/kirin-test-types/src/unit_type.rs:39` -- `impl TypeLattice for UnitType {}` (UnitType derives `Default`).

3. `crates/kirin-ir/src/signature/semantics.rs:198` -- test-internal `SimpleType` with `#[derive(Default)]` and `#[default] Mid`.

4. `crates/kirin-chumsky/src/function_text/tests.rs:54` -- test macro-generated types with `Default` impls.

**Who calls `.default()` on a TypeLattice type?**

No call to `.default()` was found in production code that depends on the `TypeLattice: Default` bound. The `Default` bound is vestigial -- it was likely added for convenience during initial development but is not exercised through the `TypeLattice` contract.

### Existing `Default` impls on implementors

- `SimpleType::default()` returns `Self::bottom()` -- correct lattice semantics but redundant with `HasBottom`.
- `UnitType` derives `Default` -- trivially returns `UnitType`, which is also `bottom()` and `top()`.
- Test `SimpleType` in semantics.rs uses `#[default] Mid` -- this is NOT `bottom()`, demonstrating the ambiguity.

### Impact of removal

Since no production code calls `.default()` through the `TypeLattice` bound, removing `Default` from the supertrait list has no functional impact on production code. Test types that implement `Default` can keep their `Default` impls (they are still useful for other purposes), but they are no longer required by `TypeLattice`.

## Proposed Design

### Change

```rust
// Before
pub trait TypeLattice: FiniteLattice + CompileTimeValue + Default {}

// After
pub trait TypeLattice: FiniteLattice + CompileTimeValue {}
```

### Caller updates

No production code updates required. The following test code needs `Default` removed from local `TypeLattice` implementations or kept as an independent impl:

1. `crates/kirin-ir/src/signature/semantics.rs:150` -- `SimpleType` derives `Default`. Keep `#[derive(Default)]` since it does not come from `TypeLattice` anymore.
2. `crates/kirin-test-types/src/simple_type.rs:55` -- `impl Default for SimpleType` can remain as a standalone impl, no longer required by the trait.
3. `crates/kirin-test-types/src/unit_type.rs` -- `#[derive(Default)]` can remain.
4. `crates/kirin-chumsky/src/function_text/tests.rs` -- test macro generates `impl Default`; remove from macro if only present for `TypeLattice`.

## Implementation Steps

1. Remove `+ Default` from the `TypeLattice` trait definition in `crates/kirin-ir/src/lattice.rs:59`.
2. Run `cargo build --workspace` to verify no compilation errors.
3. Audit test types: if any `Default` impl only existed to satisfy `TypeLattice`, remove it. If it serves other purposes (construction convenience), keep it.
4. Update the `kirin-chumsky/src/function_text/tests.rs` macro if it generates `Default` solely for `TypeLattice`.

## Risk Assessment

**Very low risk.** No production code calls `.default()` through `TypeLattice`. The change is purely subtractive on a trait bound. Existing `Default` impls on concrete types remain valid -- they just become independent of the `TypeLattice` contract.

The only risk is if downstream users (outside this workspace) have code that relies on `T: TypeLattice` implying `T: Default`. This is a breaking change for such users, but it is the correct semantic fix. Semver-wise, this is a minor breaking change to the trait's contract.

## Testing Strategy

- `cargo build --workspace` confirms compilation.
- `cargo nextest run --workspace` confirms no test regressions.
- `cargo test --doc --workspace` confirms doctest compatibility.
- Inspect the test macro in `function_text/tests.rs` to verify it does not break.
