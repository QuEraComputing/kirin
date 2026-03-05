# Derive Framework Cleanup Design

## Goal

Aggressively simplify kirin-derive-toolkit by completing the Template system migration: delete all legacy Scan/Emit/Generator code, rewrite the builder as a native template, migrate chumsky off the Scan trait, refactor interpreter derives to use TraitImplTemplate, and add a naming convention helper for generated identifier hygiene.

## Scope

### Delete (~2,100 lines)

| Target | Lines | Reason |
|--------|-------|--------|
| `generators/property/` (5 files) | ~350 | Replaced by `BoolProperty` method pattern |
| `generators/field/` (6 files) | ~450 | Replaced by `FieldCollection` method pattern |
| `generators/builder/` (6 files) | ~450 | Rewritten as native `BuilderTemplate` |
| `generators/marker.rs` | ~15 | Replaced by `MarkerTemplate` |
| `generators/common.rs` | ~49 | Bridge logic for deleted generators |
| `generators/mod.rs` | ~20 | Empty after cleanup |
| `emit.rs` | ~196 | Zero downstream users |
| `scan.rs` | ~238 | Only user (chumsky) migrated to direct iteration |
| `generator.rs` | ~90 | `Generator<L>` trait replaced by `Template<L>` |
| `derive/` directory | ~70 | Merged into `context/` |

### Rewrite 1: Native BuilderTemplate

The builder generates three kinds of output that don't fit `MethodPattern`:

1. **Constructor functions** — `fn op_add(stage, lhs, rhs) -> AddResult` per variant, or `new()` for structs
2. **Result wrapper module** — hidden `{name}_build_result` module with result structs + `From<Result> -> Statement`
3. **Wrapper `From` impls** — `impl From<WrappedType> for MyEnum` for `#[wraps]` variants

Approach: Rewrite `BuilderTemplate` as a self-contained `Template<StandardLayout>` that iterates `DeriveContext.statements` directly, using `ConstructorBuilder` from `codegen/`. The logic from `generators/builder/helpers.rs` (build_fn_body, build_fn_inputs, result naming, SSA creation) moves into the template. No Scan/Emit needed.

### Rewrite 2: Chumsky ValueTypeScanner

Replace the `Scan` trait impl (`ValueTypeScanner`) with a standalone function:

```rust
pub fn collect_value_types_needing_bounds(
    input: &Input<ChumskyLayout>,
    generics: &syn::Generics,
) -> Vec<syn::Type>
```

Direct iteration over `input.data` statements and fields, filtering by `FieldCategory::Value`. ~20 lines replacing ~80 lines + the entire Scan trait dependency.

### Rewrite 3: Interpreter derives → TraitImplTemplate

**Interpretable:** Replace manual `TraitImpl` + `MatchExpr` closure with:
```rust
TraitImplTemplate::new(interpretable_path, crate_path)
    .generics_modifier(|g| { /* add '__ir, __InterpI, __InterpL */ })
    .trait_generics(|ctx| quote! { <'__ir, __InterpI, __InterpL> })
    .where_clause(|ctx| { /* I: Interpreter, wrapper bounds */ })
    .method(MethodSpec {
        name: interpret_ident,
        pattern: Box::new(DelegateToWrapper::new(trait_fn, method).require_all()),
        ..
    })
```

**CallSemantics:** Replace manual match building with `SelectiveDelegation`:
```rust
TraitImplTemplate::new(call_semantics_path, crate_path)
    .method(MethodSpec {
        name: eval_call_ident,
        pattern: Box::new(SelectiveDelegation::new(
            trait_fn, method, "callable", check_global, fallback
        )),
        ..
    })
```

### Relocate

- `generators/stage_info.rs` → `stage_info.rs` (top-level, self-contained, future work for template migration)
- `derive/input.rs` (`InputMeta`, `PathBuilder`) → merged into `context/`

### New: Naming Convention Helper

Add a `Hygiene` struct to the template system for consistent generated name prefixing:

```rust
pub struct Hygiene {
    prefix: String, // e.g. "kirin"
}

impl Hygiene {
    pub fn new(prefix: &str) -> Self;
    pub fn ident(&self, name: &str) -> syn::Ident;      // __kirin_name
    pub fn lifetime(&self, name: &str) -> syn::Lifetime;  // '__kirin_name
    pub fn type_ident(&self, name: &str) -> syn::Ident;   // __KirinName (CamelCase)
}
```

Templates use `Hygiene` instead of ad-hoc `__` naming. Provides consistent prefix, easy to grep, zero runtime cost.

## Final Module Structure

```
crates/kirin-derive-toolkit/src/
├── ir/              # UNCHANGED: Layout, Input<L>, Statement<L>, FieldInfo
├── template/        # Template system
│   ├── mod.rs       #   Template<L>, CompositeTemplate, TemplateBuilder
│   ├── trait_impl.rs#   TraitImplTemplate + factory methods
│   ├── builder_template.rs  # REWRITTEN: native builder generation
│   ├── field_iter_set.rs
│   ├── inherent_impl.rs
│   ├── type_def.rs
│   └── method_pattern/
│       ├── mod.rs
│       ├── bool_property.rs
│       ├── delegate.rs
│       ├── field_collection.rs
│       ├── builder_pattern.rs
│       └── custom.rs
├── tokens/          # UNCHANGED: TraitImpl, MatchExpr, Pattern, etc.
├── codegen/         # UNCHANGED: ConstructorBuilder, FieldBindings, etc.
├── context/         # MERGED: DeriveContext + InputMeta + PathBuilder
├── stage.rs         # UNCHANGED: stage attribute parsing
├── stage_info.rs    # RELOCATED from generators/
├── misc.rs          # UNCHANGED: utilities
├── hygiene.rs       # NEW: naming convention helper
├── test_util.rs     # UNCHANGED
└── lib.rs           # SIMPLIFIED prelude (no legacy re-exports)
```

### DELETED
```
├── scan.rs          # DELETED
├── emit.rs          # DELETED
├── generator.rs     # DELETED
├── derive/          # DELETED (merged into context/)
└── generators/      # DELETED (entire directory)
```

## Crate Impact

| Crate | Changes |
|-------|---------|
| `kirin-derive-toolkit` | Major: deletions, builder rewrite, context merge, hygiene module |
| `kirin-derive-ir` | Minor: update `stage_info` import path |
| `kirin-derive-interpreter` | Medium: rewrite to use `TraitImplTemplate` + method patterns |
| `kirin-derive-chumsky` | Minor: replace `ValueTypeScanner` with direct iteration function |
| `kirin-derive-prettyless` | None |

## Verification

```bash
cargo build --workspace
cargo nextest run --workspace
cargo test --doc --workspace
```
