# Implementer -- Code Quality Review: kirin-derive-toolkit

## Clippy Workaround Audit

| Location | Allow Type | Reason | Classification | Action |
|----------|-----------|--------|---------------|--------|
| `src/ir/input.rs:111` | `allow(clippy::large_enum_variant)` | `Data<L>` enum has `Struct(DataStruct<L>)` vs `Enum(DataEnum<L>)`. `DataEnum` contains a Vec of variants and is significantly larger than `DataStruct`. | genuinely needed | Keep -- boxing the large variant would add indirection for the common case. The enum is used in derive macro processing (not hot path). |
| `src/ir/fields/data.rs:40` | `allow(clippy::large_enum_variant)` | `FieldData<L>` enum has small variants (`Block`, `Successor`, etc.) and a large `Value` variant containing `syn::Type`, optional `syn::Expr`, etc. | genuinely needed | Keep -- same reasoning. Derive macro data structures are constructed once and processed linearly. Boxing would add complexity without meaningful benefit. |

## Logic Duplication

### 1. Template method patterns (P3, confirmed)

The template system (`src/template/`) has multiple `MethodPattern` implementations (bool_property, field_collection, delegate, custom, builder_pattern) that each implement `generate_for_struct` and `generate_for_variant`. This is the deliberate extension pattern of the template system. The trait structure is well-designed for composability.

No significant duplication was found within the template module -- each pattern generates genuinely different code.

### 2. Tokens module delegation patterns (P3, confirmed)

**Files:** `src/tokens/delegation.rs`, `src/tokens/trait_impl.rs`, `src/tokens/match_expr.rs`

These modules generate `quote!` TokenStreams for different code patterns. The code generation is inherently repetitive (lots of `quote!` blocks with minor variations), but the actual logic differs between delegation, trait impl, and match expression generation. No actionable duplication.

## Rust Best Practices

### Missing `#[must_use]` annotations (P2, confirmed)

Zero `#[must_use]` in the crate. Less critical here since this is derive macro infrastructure (consumed by proc-macro expansion, not user-facing runtime code). However, `Input::compose()` returns a builder that is useless if not `.build()`-ed.

### Generous use of `.clone()` on syn types (P3, confirmed)

Proc-macro code typically clones `syn::Ident`, `syn::Path`, `syn::Type`, etc. frequently because `quote!` consumes values. This is expected and correct for derive macro codegen -- `syn` types are designed to be cheaply cloneable.

### `pub(crate)` vs `pub` visibility (P3, confirmed)

Several types in the `ir` module are `pub` but are only used within the derive ecosystem (not by end users of kirin). Since derive-toolkit is consumed by other derive crates, `pub` is appropriate for cross-crate access within the workspace. No issue.

### Error handling in derive context (P3, confirmed)

The crate uses `darling::Error` for attribute parsing errors, which provides good diagnostics. No `unwrap()` or `expect()` calls in non-test code paths. Error propagation is clean.

## Summary

- P2 confirmed -- Missing `#[must_use]` (low impact for derive infrastructure)
- P3 confirmed -- Both `large_enum_variant` allows are genuinely needed for derive data structures
- P3 confirmed -- No significant logic duplication; template system is well-factored
