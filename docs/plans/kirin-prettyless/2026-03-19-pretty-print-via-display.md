# `PrettyPrintViaDisplay` Marker Trait

## Problem

Many `PrettyPrint` implementations are pure boilerplate that simply delegates to `Display`:

```rust
impl PrettyPrint for ArithType {
    fn namespaced_pretty_print<'a, L: Dialect + PrettyPrint>(
        &self,
        doc: &'a Document<'a, L>,
        _namespace: &[&str],
    ) -> ArenaDoc<'a>
    where
        L::Type: std::fmt::Display,
    {
        doc.text(self.to_string())
    }
}
```

This is 10 lines of ceremony for 1 line of logic. The `L`, `namespace`, and `L::Type: Display` bound are all irrelevant to leaf types.

## Research Findings

### Inventory of `doc.text(self.to_string())` impls

**In `kirin-prettyless/src/impls.rs`:**
- `Successor` (line 112)
- `Symbol` (line 127, with lookup -- NOT plain Display, excluded)
- `GlobalSymbol` (line 149, with lookup -- excluded)
- `i8, i16, i32, i64, isize, u8, u16, u32, u64, usize` (10 types via `impl_pretty_print_int!` macro, line 225)
- `bool` (line 280, uses conditional -- close but not exactly `to_string()`)
- `String` (line 293, uses `{:?}` -- NOT plain Display)

**In external crates:**
- `ArithType` in `kirin-arith/src/types/arith_type.rs`
- `ArithValue` in `kirin-arith/src/types/arith_value.rs`
- `Value` in `kirin-test-types/src/value.rs` (2 impls for inner types)

**Count of direct candidates** (exact `doc.text(self.to_string())` pattern):
- 10 integer types (via macro)
- `Successor`
- `ArithType`
- `ArithValue`
- `Value` (2)
- Total: ~15 impls

**Near-candidates** (could adapt):
- `f32`/`f64`: use conditional formatting for decimal point -- NOT candidates
- `bool`: hardcoded strings -- NOT a candidate (though `bool::to_string()` would work)
- `String`: uses `{:?}` for quoting -- NOT a candidate

### Trait design constraints

A blanket `impl<T: Display> PrettyPrint for T` would conflict with:
- `impl<T: PrettyPrint> PrettyPrint for Vec<T>` (Vec doesn't impl Display, so no direct conflict, but future impls could)
- More critically, it would conflict with derive-generated `PrettyPrint` impls on types that also implement `Display`

Therefore, a **marker trait** is the correct approach -- opt-in, no coherence issues.

### Coherence/orphan analysis

`PrettyPrintViaDisplay` would be defined in `kirin-prettyless`. The blanket impl:
```rust
impl<T: PrettyPrintViaDisplay + Display> PrettyPrint for T { ... }
```

Coherence is fine because:
- The blanket impl is in the same crate as `PrettyPrint` (kirin-prettyless)
- External crates can implement `PrettyPrintViaDisplay` on their own types
- No overlap with existing impls: the `impl_pretty_print_int!` macro impls would be replaced by `PrettyPrintViaDisplay` marker impls

For `kirin-prettyless`'s own types like `Successor`: the existing impl can be replaced by `impl PrettyPrintViaDisplay for Successor {}`.

For `kirin-ir` types: `Successor` is defined in `kirin-ir`, and `PrettyPrintViaDisplay` in `kirin-prettyless`. The impl `impl PrettyPrintViaDisplay for Successor` would go in `kirin-prettyless` (orphan rule satisfied: trait is local). This is the same pattern as the existing manual `PrettyPrint` impl.

For external types like `i32`: `impl PrettyPrintViaDisplay for i32` in `kirin-prettyless` is fine (trait is local).

For user types like `ArithType`: `impl PrettyPrintViaDisplay for ArithType` in `kirin-arith` is fine (type is local).

### Interaction with derives

The `#[derive(PrettyPrint)]` macro in `kirin-derive-chumsky` generates `PrettyPrint` impls directly. If we add `PrettyPrintViaDisplay`, the derive should NOT change -- it generates format-string-based impls, not Display-based impls. The marker trait is for types that implement `PrettyPrint` manually via Display delegation.

However, the type-enum derive plan (see `2026-03-19-type-enum-derive.md`) could generate `PrettyPrintViaDisplay` instead of `PrettyPrint` for type enums, since they always use `doc.text(self.to_string())`.

## Proposed Design

### Marker trait

```rust
// In kirin-prettyless/src/traits.rs

/// Marker trait for types whose `PrettyPrint` implementation is just
/// `doc.text(self.to_string())`. Implement this (empty) trait on your type
/// to get a blanket `PrettyPrint` impl.
///
/// # Requirements
/// - The type must implement `Display`.
/// - The type must NOT have a manual `PrettyPrint` impl (would conflict).
///
/// # Example
/// ```ignore
/// impl PrettyPrintViaDisplay for MyType {}
/// // Now MyType: PrettyPrint, rendering via Display::fmt
/// ```
pub trait PrettyPrintViaDisplay: std::fmt::Display {}
```

### Blanket impl

```rust
// In kirin-prettyless/src/traits.rs

impl<T: PrettyPrintViaDisplay> PrettyPrint for T {
    fn namespaced_pretty_print<'a, L: Dialect + PrettyPrint>(
        &self,
        doc: &'a Document<'a, L>,
        _namespace: &[&str],
    ) -> ArenaDoc<'a>
    where
        L::Type: std::fmt::Display,
    {
        doc.text(self.to_string())
    }
}
```

### Migrations

Replace in `kirin-prettyless/src/impls.rs`:
```rust
// Before (10 impls via macro + Successor)
macro_rules! impl_pretty_print_int { ... }
impl_pretty_print_int!(i8, i16, i32, i64, isize, u8, u16, u32, u64, usize);
impl PrettyPrint for Successor { ... }

// After
macro_rules! impl_pretty_print_via_display {
    ($($ty:ty),*) => { $(impl PrettyPrintViaDisplay for $ty {})* };
}
impl_pretty_print_via_display!(i8, i16, i32, i64, isize, u8, u16, u32, u64, usize);
impl PrettyPrintViaDisplay for Successor {}
```

External crates:
```rust
// kirin-arith/src/types/arith_type.rs
impl PrettyPrintViaDisplay for ArithType {}
// Remove the 10-line manual PrettyPrint impl
```

## Implementation Steps

1. **Define `PrettyPrintViaDisplay`** in `kirin-prettyless/src/traits.rs`.
2. **Add blanket `PrettyPrint` impl** for `PrettyPrintViaDisplay` types.
3. **Re-export** `PrettyPrintViaDisplay` from `kirin-prettyless` and its prelude.
4. **Migrate integer types** in `impls.rs`: replace `impl_pretty_print_int!` macro with `impl PrettyPrintViaDisplay`.
5. **Migrate `Successor`** in `impls.rs`.
6. **Migrate `ArithType`** in `kirin-arith`.
7. **Migrate `ArithValue`** in `kirin-arith`.
8. **Migrate `Value`** in `kirin-test-types` (feature-gated behind `pretty`).
9. **Update `UnitType`** -- note: `UnitType` prints `"()"` not `self.to_string()` which also returns `"()"`, so it IS a candidate.
10. **Keep `f32`, `f64`, `bool`, `String`** as manual impls (they have custom formatting logic).

## Risk Assessment

**Low risk:**
- Marker trait + blanket impl is a standard Rust pattern with no coherence surprises.
- The blanket impl is in the same crate as the trait, avoiding orphan rule issues.
- Migration is mechanical: replace N-line impls with 1-line marker impls.

**No risk of breakage:**
- Existing code that imports `PrettyPrint` does not need changes.
- Types that already have manual `PrettyPrint` impls continue to work (they just don't use the marker).
- The marker is purely opt-in.

**One consideration:**
- If a type implements `PrettyPrintViaDisplay` AND tries to `#[derive(PrettyPrint)]`, there will be a conflicting impl error. This is a compile-time error with a clear message. The derive should check for `PrettyPrintViaDisplay` and warn, but this is not strictly necessary since the Rust compiler already catches it.

## Testing Strategy

- **Unit test**: Implement `PrettyPrintViaDisplay` on a test type, verify `pretty_print` produces the Display output.
- **Existing roundtrip tests**: After migrating `ArithType`, run `cargo nextest run -p kirin-arith` and the roundtrip tests in `tests/roundtrip/arith.rs` to verify no regression.
- **Compile-fail test** (optional): Verify that implementing both `PrettyPrintViaDisplay` and manual `PrettyPrint` produces a clear error.
- **Doc test**: Add a doc example on the trait showing the one-line usage pattern.
