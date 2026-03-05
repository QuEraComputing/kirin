+++
rfc = "0014"
title = "derive-toolkit template architecture"
status = "Implemented"
authors = ["Roger-luo <code@rogerluo.dev>"]
created = "2026-03-05T18:05:18.586614Z"
last_updated = "2026-03-05T18:05:18.586614Z"
+++

# RFC 0014: derive-toolkit template architecture

## Summary

Replace the two competing code generation approaches in `kirin-derive-toolkit` — the Scan/Emit visitor pattern and the `Generator` trait — with a single **Template** system. Templates are composable building blocks where each template handles code structure (trait impls, type definitions) and pluggable `MethodPattern` objects handle per-variant logic. This reduces the public API surface, eliminates duplicated pattern-building logic across generators, and makes it straightforward to add new derive macros.

## Motivation

- Problem: The derive toolkit had two competing codegen approaches with no bridge between them. The `Scan`/`Emit` visitor pattern (`scan.rs`, `emit.rs`) required two-pass traversal with mutable state accumulation. The `Generator` trait (`generator.rs`, `generators/`) provided higher-level abstractions but was hardcoded to `StandardLayout`, couldn't compose, and carried per-generator state types (`StatementInfo` structs for property, field, builder). Pattern-building logic (e.g., wrapper delegation, field iteration, bool property reads) was duplicated across generator implementations. The public API surface was large and confusing for downstream derive macro developers.
- Why now: Adding new derives required choosing between the two approaches and duplicating shared logic. The `kirin-derive-interpreter` crate had its own `pattern.rs` reimplementing wrapper detection already solved in the toolkit. Growing the set of derivable traits made the maintenance burden unsustainable.
- Stakeholders: `kirin-derive-toolkit`, `kirin-derive-ir`, `kirin-derive-interpreter`, derive macro authors

## Goals

- Single entry point: `Input::compose().add(template).build()` for all derive macro code generation
- Composable templates that each produce `Vec<TokenStream>` fragments from a shared `DeriveContext`
- Pluggable per-variant logic via `MethodPattern<L>` trait with pre-built patterns for common cases
- Factory methods for declarative one-liner derives (`bool_property`, `field_iter`, `marker`)
- Enriched `DeriveContext`/`StatementContext` as the single source of truth (no per-generator state)
- Migrate `kirin-derive-ir` and `kirin-derive-interpreter` to the new system

## Non-goals

- Rewriting `kirin-derive-chumsky` — it uses its own codegen approach (multi-output AST + parser + emitter) with `Input<ChumskyLayout>` directly
- Removing `Scan`/`Emit` traits entirely — they remain for `kirin-derive-chumsky` backward compatibility
- Removing `generators/` directory entirely — `BuilderTemplate` wraps the existing `DeriveBuilder`, and `stage_info::generate` is still used by `StageMeta` derives

## Guide-level Explanation

Derive macros now follow a three-layer model:

**Layer 1 — Declarative** (factory methods for common patterns):
```rust
TraitImplTemplate::bool_property(
    BoolPropertyConfig { kind: PropertyKind::IsPure, trait_name: "IsPure", trait_method: "is_pure" },
    "::kirin::ir",
)
```

**Layer 2 — Composition** (template + method pattern):
```rust
TraitImplTemplate::new(trait_path, crate_path)
    .method(MethodSpec {
        name: format_ident!("interpret"),
        self_arg: quote! { &self },
        params: vec![quote! { interp: &mut I }],
        return_type: Some(quote! { ... }),
        pattern: Box::new(DelegateToWrapper::new(trait_fn, method_ident).require_all()),
    })
```

**Layer 3 — Custom** (closures for one-off logic):
```rust
Custom::separate(
    |ctx, stmt_ctx| { /* struct body */ },
    |ctx, stmt_ctx| { /* variant body */ },
)
```

All three compose through the same builder:
```rust
let ir = Input::<StandardLayout>::from_derive_input(&ast)?;
ir.compose()
    .add(TraitImplTemplate::bool_property(IS_PURE, crate_path))
    .add(TraitImplTemplate::field_iter(HAS_ARGS, crate_path, "'__args"))
    .add(TraitImplTemplate::marker(&trait_path, &ir_type))
    .add(BuilderTemplate::new(crate_path))
    .build()
```

## Reference-level Explanation

### Core trait: `Template<L>`

Defined in `crates/kirin-derive-toolkit/src/template/mod.rs`:

```rust
pub trait Template<L: Layout> {
    fn emit(&self, ctx: &DeriveContext<'_, L>) -> darling::Result<Vec<TokenStream>>;
}
```

A blanket impl allows closures `Fn(&DeriveContext<'_, L>) -> darling::Result<Vec<TokenStream>>` to serve as templates.

### Template types

| Type | File | Purpose |
|------|------|---------|
| `TraitImplTemplate<L>` | `template/trait_impl.rs` | `impl Trait for Type { methods, assoc types }` |
| `MarkerTemplate` | `template/trait_impl.rs` | `impl Dialect for Type { type Type = ...; }` |
| `FieldIterTemplateSet` | `template/field_iter_set.rs` | Composite: trait impl + iterator type def + Iterator impl |
| `BuilderTemplate` | `template/builder_template.rs` | Wraps existing `DeriveBuilder` as a Template |
| `InherentImplTemplate<L>` | `template/inherent_impl.rs` | `impl Type { methods }` (closure-based) |
| `TypeDefTemplate<L>` | `template/type_def.rs` | Type definitions (closure-based) |
| `CompositeTemplate<L>` | `template/mod.rs` | Groups multiple templates |

### MethodPattern trait and pre-built patterns

Defined in `crates/kirin-derive-toolkit/src/template/method_pattern/mod.rs`:

```rust
pub trait MethodPattern<L: Layout> {
    fn for_struct(&self, ctx: &DeriveContext<'_, L>, stmt: &StatementContext<'_, L>) -> darling::Result<TokenStream>;
    fn for_variant(&self, ctx: &DeriveContext<'_, L>, stmt: &StatementContext<'_, L>) -> darling::Result<TokenStream>;
    fn extra_bounds(&self, _ctx: &DeriveContext<'_, L>, _stmt: &StatementContext<'_, L>) -> Vec<syn::WherePredicate> { vec![] }
}
```

| Pattern | File | Purpose |
|---------|------|---------|
| `BoolProperty` | `method_pattern/bool_property.rs` | Read bool from `#[kirin(attr)]` or `#[bare_attr]`, with defaults and validation |
| `DelegateToWrapper<L>` | `method_pattern/delegate.rs` | Forward through `#[wraps]` field via `<Wrapped as Trait>::method(...)` |
| `SelectiveDelegation<L>` | `method_pattern/delegate.rs` | Delegate only for attr-marked variants (e.g., `#[callable]`), fallback for rest |
| `FieldCollection` | `method_pattern/field_collection.rs` | Chain `iter()`/`iter_mut()` calls for a field category (Arguments, Results, Blocks, etc.) |
| `Custom<L>` | `method_pattern/custom.rs` | Closure escape hatch with `new()` (same for both) or `separate()` (different closures) |

### DeriveContext enrichment

`StatementContext` in `crates/kirin-derive-toolkit/src/context.rs` gained two new fields:

```rust
pub wrapper_type: Option<&'ir syn::Type>,
pub wrapper_binding: Option<proc_macro2::TokenStream>,
```

These pre-compute wrapper access info from the `#[wraps]` attribute so that method patterns do not need to scan for it themselves. This consolidated the duplicated wrapper-detection logic that existed in `kirin-derive-interpreter/src/pattern.rs` and across generator implementations.

### TemplateBuilder composition

```rust
impl<L: Layout> Input<L> {
    pub fn compose(&self) -> TemplateBuilder<'_, L>;
}
impl<'ir, L: Layout> TemplateBuilder<'ir, L> {
    pub fn add(self, t: impl Template<L> + 'static) -> Self;
    pub fn context(&self) -> &DeriveContext<'ir, L>;
    pub fn build(self) -> darling::Result<TokenStream>;
}
```

`build()` runs all templates sequentially, accumulates errors via `darling::Error::accumulator()`, and returns the combined `TokenStream`.

### Crate impact matrix

| Crate | Impact | Tests to update |
|-------|--------|-----------------|
| `kirin-derive-toolkit` | Major: new `template/` module (12 files), enriched `DeriveContext` | All existing tests pass unchanged |
| `kirin-derive-ir` | Medium: rewrote `derive_statement` to use `TemplateBuilder` | `cargo nextest run -p kirin-ir` |
| `kirin-derive-interpreter` | Medium: rewrote `Interpretable` and `CallSemantics` derives, deleted `pattern.rs` and scan/emit files | `cargo nextest run -p kirin-interpreter` |
| `kirin-derive-chumsky` | None: still uses `Scan`/`Emit` with `Input<ChumskyLayout>` | No changes |
| `kirin-derive-prettyless` | None: uses `stage::parse_stage_variants` directly | No changes |

## Drawbacks

- **Legacy code retained**: The `generators/` module, `scan.rs`, `emit.rs`, and `generator.rs` remain because `BuilderTemplate` wraps `DeriveBuilder`, `StageMeta` uses `stage_info::generate`, and `kirin-derive-chumsky` uses `Scan`/`Emit`. This means two codegen approaches coexist in the toolkit.
- **Closure-based templates lack type safety**: `InherentImplTemplate` and `TypeDefTemplate` are thin closure wrappers. They don't enforce structural invariants the way `TraitImplTemplate` does.
- **Learning curve**: Developers must understand the Template/MethodPattern split and when to use factory methods vs. composition vs. closures.

## Rationale and Alternatives

### Proposed approach rationale

- Templates compose naturally via `TemplateBuilder::add()` — no coupling between templates
- `MethodPattern` captures the most common variation point (per-variant logic) as a reusable abstraction
- Factory methods (`bool_property`, `field_iter`, `marker`) cover ~80% of derive macro needs with one-liners
- Closures serve as an escape hatch for complex cases (interpreter derives) without forcing everything through the declarative API
- `DeriveContext` as single source of truth eliminates per-generator state types

### Alternative: Extend the Generator trait

- Description: Keep the existing `Generator<L>` trait and add missing features (composition, generic layout support)
- Pros: No new abstractions; incremental change
- Cons: Generator's two-pass Scan/Emit model is inherently stateful, making composition difficult. Each generator owns its `StatementInfo` types, preventing shared context. Adding generic layout support would require rewriting most generators anyway.
- Reason not chosen: The fundamental design (mutable state accumulation across passes) made composition impractical without a rewrite that would be equivalent to the template system.

### Alternative: Macro-based code generation

- Description: Use declarative macros or build scripts to generate derive implementations
- Pros: No runtime trait objects; fully expanded at compile time
- Cons: Declarative macros can't inspect types or attributes. Build scripts add complexity and can't participate in proc-macro expansion. Neither approach handles the conditional logic (wrapper delegation, attribute reading, field categorization) that drives most derives.
- Reason not chosen: The problem domain requires programmatic inspection of derive input, which proc-macro code (syn/quote) handles well.

## Prior Art

- **MLIR TableGen**: MLIR uses a declarative TableGen approach where operation definitions specify traits, and C++ code is generated from those definitions. The template system is analogous: declarative configs (factory methods) generate trait impls, with escape hatches for custom logic.
- **Rust derive macro ecosystem**: Libraries like `derive_more` and `strum` use per-trait derive implementations. Kirin's template system is a framework for building such derives with shared infrastructure.
- **syn/darling**: The toolkit builds on `syn` for AST parsing and `darling` for attribute parsing, following established Rust proc-macro conventions.

## Backward Compatibility and Migration

- Breaking changes: None for end users of derive macros (`#[derive(Dialect)]`, `#[derive(Interpretable)]`, etc.). The derive macro interface is unchanged.
- Internal breaking changes: `kirin-derive-ir` and `kirin-derive-interpreter` entry points were rewritten. Direct users of the toolkit's `Generator` trait, `GenerateBuilder`, or pre-built generators would need to migrate to templates.
- Migration steps:
  1. Replace `Generator` + `GenerateBuilder` with `Input::compose().add(...).build()`
  2. Replace `DeriveProperty` usage with `TraitImplTemplate::bool_property()`
  3. Replace `DeriveFieldIter` usage with `TraitImplTemplate::field_iter()`
  4. Replace `DeriveBuilder` usage with `BuilderTemplate::new()`
  5. Replace per-generator `StatementInfo` with `StatementContext` fields
- Compatibility strategy: Legacy `Scan`/`Emit` traits remain public for `kirin-derive-chumsky`. Old generator infrastructure is kept (not deleted) but no longer used by `kirin-derive-ir` or `kirin-derive-interpreter`.

## How to Teach This

- The `template/mod.rs` module doc explains the three-layer model with an example
- New derive macros should start with factory methods, escalate to composition, and use closures only for genuinely custom logic
- The prelude re-exports all essential types: `Template`, `TemplateBuilder`, `TraitImplTemplate`, `MethodPattern`, `MethodSpec`, `Custom`, etc.
- Existing derive crates (`kirin-derive-ir`, `kirin-derive-interpreter`) serve as reference implementations

## Reference Implementation Plan

Implementation was completed in a single pass:

1. Enrich `DeriveContext` — add `wrapper_type`, `wrapper_binding` to `StatementContext`
2. Build `template/` module — `Template` trait, `TemplateBuilder`, `CompositeTemplate`, blanket closure impl
3. Build template types — `TraitImplTemplate`, `MarkerTemplate`, `FieldIterTemplateSet`, `BuilderTemplate`, `InherentImplTemplate`, `TypeDefTemplate`
4. Build method patterns — `BoolProperty`, `DelegateToWrapper`, `SelectiveDelegation`, `FieldCollection`, `Custom`
5. Add factory methods — `TraitImplTemplate::bool_property()`, `::field_iter()`, `::marker()`
6. Migrate `kirin-derive-ir` — rewrite `derive_statement` entry point
7. Migrate `kirin-derive-interpreter` — rewrite `Interpretable` + `CallSemantics`, delete `pattern.rs` and scan/emit files
8. Update prelude — export new types alongside legacy re-exports

### Acceptance Criteria

- [x] All 205 nextest tests pass
- [x] All 9 doctests pass
- [x] Full workspace builds cleanly
- [x] `kirin-derive-ir` uses template builder exclusively
- [x] `kirin-derive-interpreter` uses template builder exclusively
- [x] Net code reduction (~343 lines removed)

## Unresolved Questions

- Should `BuilderTemplate` be rewritten as a native template instead of wrapping `DeriveBuilder`? The current wrapper works but carries legacy state management.
- Should `kirin-derive-chumsky` be migrated to `Template<ChumskyLayout>` to enable full removal of `Scan`/`Emit`? This is a separate effort with its own complexity.
- Should `StageMeta` derive be migrated from `generators::stage_info::generate` to a template? It has a unique codegen pattern that doesn't fit `TraitImplTemplate` naturally.

## Future Possibilities

- Migrate `kirin-derive-chumsky` to templates, enabling deletion of `Scan`/`Emit` and the entire `generators/` directory
- Add a `ValidateTemplate` that runs validation-only logic (no code generation) as a composable pre-check
- Template-level error recovery: allow individual templates to fail without aborting the entire derive, collecting all errors for a better developer experience
- Code generation debugging: extend the existing `KIRIN_DEBUG_DERIVE` dump to show which template produced each fragment

## Revision Log

| Date | Change |
| --- | --- |
| 2026-03-05 | RFC created retroactively after implementation |
