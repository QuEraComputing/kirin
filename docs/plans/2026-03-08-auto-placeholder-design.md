# Auto-Placeholder for ResultValue Fields

## Problem

After replacing `Default` on `Dialect::Type` with a `Placeholder` trait, every dialect with `ResultValue` fields requires:

1. `T: CompileTimeValue + Placeholder` bounds on struct/enum definitions
2. `#[kirin(type = T::placeholder())]` annotation on every `ResultValue` field
3. `+ Placeholder` bounds cascading to interpret_impl, tests, and downstream code

This is verbose and error-prone. The vast majority of `ResultValue` fields use the same pattern: `T::placeholder()`.

## Design: Derive-Inferred Placeholder Defaults

### Core Rule

When a dialect has `#[kirin(type = T)]` at enum/struct level and contains `ResultValue` fields without an explicit `#[kirin(type = ...)]` annotation, the derive:

1. Auto-generates `T::placeholder()` as the type expression for that field
2. Auto-adds `T: Placeholder` to generated `where` clauses (builder functions, EmitIR impls)

Explicit `#[kirin(type = expr)]` overrides the default — no automatic Placeholder bound is added for that field (e.g., `Constant` uses `value.type_of()`).

### Before / After

Before:
```rust
#[kirin(pure, fn, type = T)]
pub enum Arith<T: CompileTimeValue + Placeholder> {
    Add {
        lhs: SSAValue,
        rhs: SSAValue,
        #[kirin(type = T::placeholder())]
        result: ResultValue,
        #[kirin(default)]
        marker: PhantomData<T>,
    },
}
```

After:
```rust
#[kirin(pure, fn, type = T)]
pub enum Arith<T: CompileTimeValue> {
    Add {
        lhs: SSAValue,
        rhs: SSAValue,
        result: ResultValue,
        #[kirin(default)]
        marker: PhantomData<T>,
    },
}
```

### Scope of Changes

**Derive toolkit** (`kirin-derive-toolkit`):

- `ir/statement/definition.rs` — ResultValue without `#[kirin(type = ...)]` defaults to `<ir_type>::placeholder()` instead of erroring. `ir_type` comes from the enum-level `#[kirin(type = T)]`.
- `ir/fields/data.rs` — `FieldData::Result` gains `is_auto_placeholder: bool` to track whether the default was auto-generated.
- `template/builder_template/helpers.rs` — When any ResultValue has `is_auto_placeholder: true`, add `T: Placeholder` to the builder's where clause.

**Parser derive** (`kirin-derive-chumsky`):

- `codegen/emit_ir/generate.rs` and `self_emit.rs` — Same conditional Placeholder bound logic for EmitIR impls.

**Dialect crates** (migration):

- Remove `+ Placeholder` from struct/enum type bounds
- Remove `#[kirin(type = T::placeholder())]` from ResultValue fields
- Remove `+ Placeholder` from interpret_impl trait bounds
- Remove `impl Placeholder for UnitTy` from test files that don't use builders
- `kirin-constant` keeps its explicit `#[kirin(type = value.type_of())]` — no change

### Design Constraints

- **Placeholder is construction-only.** Type constraints (e.g., "Add's result type equals its input types") are enforced by abstract interpreter passes on the type lattice, not by the derive system.
- **Interpret impls don't need Placeholder.** They read SSA values and return Continuations — they never construct SSA values with placeholder types.
- **The Placeholder bound only appears in derive-generated code** (builders and parsers), never in user-written struct definitions or interpret impls.
