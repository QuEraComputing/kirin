# Derive Framework Cleanup Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Delete all legacy Scan/Emit/Generator code from kirin-derive-toolkit, rewrite BuilderTemplate as a native template, migrate chumsky off the Scan trait, refactor interpreter derives to use TraitImplTemplate, merge derive/ into context/, add naming hygiene helper.

**Architecture:** The Template system (`Template<L>` trait + `TemplateBuilder`) is already the primary codegen approach. This plan completes the migration by eliminating all legacy code paths (Scan/Emit visitors, Generator trait, pre-built generators) and consolidating the few remaining holdouts.

**Tech Stack:** Rust proc-macro infrastructure (syn, quote, darling, proc_macro2)

---

### Task 1: Add Hygiene naming helper

**Files:**
- Create: `crates/kirin-derive-toolkit/src/hygiene.rs`
- Modify: `crates/kirin-derive-toolkit/src/lib.rs`

**Step 1: Create `hygiene.rs`**

```rust
// crates/kirin-derive-toolkit/src/hygiene.rs
use proc_macro2::Span;
use quote::format_ident;

/// Generates prefixed identifiers for derive macro output to avoid
/// name collisions with user code.
///
/// All generated names use a `__{prefix}_` convention (e.g., `__kirin_ir`).
pub struct Hygiene {
    prefix: String,
}

impl Hygiene {
    pub fn new(prefix: &str) -> Self {
        Self {
            prefix: prefix.to_string(),
        }
    }

    /// Generate a snake_case identifier: `__{prefix}_{name}`
    pub fn ident(&self, name: &str) -> syn::Ident {
        format_ident!("__{}_{}", self.prefix, name)
    }

    /// Generate a lifetime: `'__{prefix}_{name}`
    pub fn lifetime(&self, name: &str) -> syn::Lifetime {
        syn::Lifetime::new(
            &format!("'__{}_{}", self.prefix, name),
            Span::call_site(),
        )
    }

    /// Generate a CamelCase type identifier: `__{Prefix}{Name}`
    pub fn type_ident(&self, name: &str) -> syn::Ident {
        let camel_prefix = crate::misc::to_camel_case(self.prefix.clone());
        let camel_name = crate::misc::to_camel_case(name.to_string());
        format_ident!("__{}{}", camel_prefix, camel_name)
    }
}
```

**Step 2: Register module and add to prelude in `lib.rs`**

In `crates/kirin-derive-toolkit/src/lib.rs`:
- Add `pub mod hygiene;` to the module list
- Add `pub use crate::hygiene::Hygiene;` to the prelude

**Step 3: Build and verify**

Run: `cargo build -p kirin-derive-toolkit`

**Step 4: Commit**

```
feat(derive-toolkit): add Hygiene naming convention helper
```

---

### Task 2: Merge `derive/` module into `context/`

This task moves `InputMeta` and `PathBuilder` from `derive/input.rs` into `context.rs`, then converts `context.rs` into a `context/` directory module.

**Files:**
- Create: `crates/kirin-derive-toolkit/src/context/mod.rs` (merge of old `context.rs` + `derive/input.rs`)
- Delete: `crates/kirin-derive-toolkit/src/context.rs`
- Delete: `crates/kirin-derive-toolkit/src/derive/input.rs`
- Delete: `crates/kirin-derive-toolkit/src/derive/mod.rs`
- Modify: `crates/kirin-derive-toolkit/src/lib.rs` (remove `pub mod derive;`, update prelude)

**Step 1: Create `context/mod.rs`**

Combine the contents of `context.rs` and `derive/input.rs` into `context/mod.rs`. The `context.rs` already imports from `derive::InputMeta`, so merge them:

- Move `InputMeta` and `PathBuilder` structs directly into `context/mod.rs`
- Keep all existing `DeriveContext` and `StatementContext` code
- Update internal imports (replace `use crate::derive::InputMeta` with direct struct reference)

**Step 2: Update `lib.rs`**

- Remove `pub mod derive;`
- The `pub mod context;` now points to `context/mod.rs`
- Update prelude: replace `pub use crate::derive::{self, InputMeta, PathBuilder}` with `pub use crate::context::{InputMeta, PathBuilder}`
- Keep `derive` re-export for backward compat if needed, or just remove it

**Step 3: Fix all internal references**

Search for `use crate::derive::` across the toolkit and replace with `use crate::context::`. Key files:
- `template/trait_impl.rs`
- `template/builder_template.rs`
- `template/field_iter_set.rs`
- `generators/builder/context.rs` (still exists temporarily)
- `generators/builder/scan.rs`
- `generators/builder/helpers.rs`
- `generators/property/context.rs`
- `generators/field/emit.rs`
- `generators/common.rs`

**Step 4: Fix downstream imports**

Search for `kirin_derive_toolkit::derive::` across all crates. Likely none since downstream crates use the prelude, but verify.

**Step 5: Build and verify**

Run: `cargo build --workspace`

**Step 6: Commit**

```
refactor(derive-toolkit): merge derive/ module into context/
```

---

### Task 3: Rewrite BuilderTemplate as native template

This is the largest task. Replace the Scan/Emit-based `DeriveBuilder` with a self-contained `Template<StandardLayout>` implementation.

**Files:**
- Rewrite: `crates/kirin-derive-toolkit/src/template/builder_template.rs`
- No other files change (kirin-derive-ir calls `BuilderTemplate::new()` which keeps the same API)

**Step 1: Rewrite `builder_template.rs`**

The new `BuilderTemplate` implements `Template<StandardLayout>` directly, iterating `DeriveContext.statements` and producing:

1. Constructor functions (per non-wrapper statement)
2. Result wrapper module (hidden, with result structs + From impl)
3. `From<WrapperType>` impls (for wrapper statements)

Port the logic from `generators/builder/helpers.rs` into the template. Key functions to port:
- `build_fn_name()` — determines constructor name
- `build_fn_inputs()` — extracts required parameters
- `build_fn_let_inputs()` — generates let-bindings with `.into()` calls
- `field_type_for_category()` — maps categories to Rust types
- `result_names()` — names result fields
- `build_fn_body()` — generates the function body
- `let_name_eq_result_value()` — creates ResultValue SSA assignments
- `build_result_impl()` — generates result struct + From<Result> impl
- `from_impl()` — generates From<WrapperType> impl

The template uses `ConstructorBuilder` from `codegen/` (unchanged) and `InputMeta` from `context/` (after Task 2 merge).

Key API: `BuilderTemplate::new()` returns `Self` with no arguments (uses crate path from `DeriveContext.meta`). The `emit()` method:

```rust
fn emit(&self, ctx: &DeriveContext<'_, StandardLayout>) -> darling::Result<Vec<TokenStream>> {
    if ctx.meta.builder.is_none() {
        return Ok(vec![]);
    }
    let crate_path = ctx.meta.path_builder(&self.default_crate_path).full_crate_path();

    match &ctx.input.data {
        Data::Struct(data) => self.emit_struct(ctx, data, &crate_path),
        Data::Enum(data) => self.emit_enum(ctx, data, &crate_path),
    }
}
```

For each statement, access fields via `stmt.collect_fields()` and wrapper info via `StatementContext`.

**Step 2: Build and test**

Run: `cargo build --workspace && cargo nextest run --workspace`

All existing derive tests must pass since the output TokenStream should be identical.

**Step 3: Commit**

```
refactor(derive-toolkit): rewrite BuilderTemplate as native template
```

---

### Task 4: Migrate chumsky ValueTypeScanner to direct iteration

**Files:**
- Modify: `crates/kirin-derive-chumsky/src/field_kind/scanner.rs`

**Step 1: Replace Scan-based scanner with direct field iteration**

The current `ValueTypeScanner` implements `Scan<'ir, ChumskyLayout>` overriding only `scan_value()`. Replace with a standalone function:

```rust
use std::collections::HashSet;
use kirin_derive_toolkit::ir::{self, Layout};
use kirin_derive_toolkit::misc::{is_type, is_type_in_generic};
use quote::quote;

use crate::ChumskyLayout;
use crate::format::{Format, FormatElement};

/// Collects Value field types that contain type parameters.
///
/// Used to determine which types need generic bounds in generated parser code.
pub fn collect_value_types_needing_bounds(
    input: &ir::Input<ChumskyLayout>,
    generics: &syn::Generics,
) -> Vec<syn::Type> {
    let type_param_names: Vec<String> = generics
        .params
        .iter()
        .filter_map(|p| {
            if let syn::GenericParam::Type(tp) = p {
                Some(tp.ident.to_string())
            } else {
                None
            }
        })
        .collect();

    if type_param_names.is_empty() {
        return Vec::new();
    }

    let mut types = Vec::new();
    let mut seen = HashSet::new();

    let statements: Vec<&ir::Statement<ChumskyLayout>> = match &input.data {
        ir::Data::Struct(data) => vec![&data.0],
        ir::Data::Enum(data) => data.variants.iter().collect(),
    };

    for stmt in statements {
        for field in stmt.iter_all_fields() {
            if field.category() != ir::fields::FieldCategory::Value {
                continue;
            }
            if let Some(ty) = field.value_type() {
                if field.has_default() {
                    continue;
                }
                for param_name in &type_param_names {
                    if is_type(ty, param_name) || is_type_in_generic(ty, param_name) {
                        let key = quote!(#ty).to_string();
                        if seen.insert(key) {
                            types.push(ty.clone());
                        }
                        break;
                    }
                }
            }
        }
    }
    types
}

// Keep fields_in_format unchanged - it doesn't use Scan
pub fn fields_in_format<L: Layout>(/* ... unchanged ... */) -> HashSet<usize> { /* ... */ }
```

**Step 2: Update `field_kind/mod.rs`**

Replace `pub use scanner::ValueTypeScanner;` with `pub use scanner::collect_value_types_needing_bounds;`.

**Step 3: Update all call sites**

Search for `ValueTypeScanner` in kirin-derive-chumsky. Update from:
```rust
let types = ValueTypeScanner::new(&generics).scan(&ir)?;
```
To:
```rust
let types = collect_value_types_needing_bounds(&ir, &generics);
```

**Step 4: Verify Statement has `iter_all_fields()`**

Check that `ir::Statement<L>` has an `iter_all_fields()` method. If not, it may be named differently — check `crates/kirin-derive-toolkit/src/ir/statement/` for the field iteration API. The Scan trait's default `scan_statement()` calls individual category methods, but `Statement` should expose all fields. If needed, iterate via `stmt.arguments().chain(stmt.results()).chain(...)` or use `stmt.collect_fields()`.

**Step 5: Build and test**

Run: `cargo build -p kirin-derive-chumsky && cargo nextest run -p kirin-chumsky`

**Step 6: Commit**

```
refactor(derive-chumsky): replace ValueTypeScanner with direct field iteration
```

---

### Task 5: Refactor Interpretable derive to use TraitImplTemplate

**Files:**
- Rewrite: `crates/kirin-derive-interpreter/src/interpretable.rs`

**Step 1: Rewrite using TraitImplTemplate + DelegateToWrapper**

Replace the closure-based approach with:

```rust
use kirin_derive_toolkit::context::DeriveContext;
use kirin_derive_toolkit::ir::{Input, StandardLayout};
use kirin_derive_toolkit::misc::from_str;
use kirin_derive_toolkit::template::TraitImplTemplate;
use kirin_derive_toolkit::template::method_pattern::{DelegateToWrapper, MethodSpec};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};

pub fn do_derive_interpretable(input: syn::DeriveInput) -> darling::Result<TokenStream> {
    let ir = Input::<StandardLayout>::from_derive_input(&input)?;

    let interp_crate: syn::Path = from_str("kirin_interpreter");
    let trait_path: syn::Path = from_str("Interpretable");

    ir.compose()
        .add(
            TraitImplTemplate::new(trait_path, interp_crate.clone())
                .generics_modifier(|g| {
                    let mut g = g.clone();
                    g.params.insert(0, syn::parse_quote!('__ir));
                    g.params.push(syn::parse_quote!(__InterpI: #interp_crate::Interpreter<'__ir>));
                    g.params.push(syn::parse_quote!(__InterpL: #interp_crate::prelude::Dialect));
                    g
                })
                .trait_generics(|_ctx| quote! { <'__ir, __InterpI, __InterpL> })
                .where_clause(|ctx| {
                    // Add bounds: each wrapper type must impl Interpretable
                    let mut predicates = Vec::new();
                    for stmt_ctx in ctx.statements.values() {
                        if let Some(wrapper_ty) = stmt_ctx.wrapper_type {
                            predicates.push(syn::parse_quote! {
                                #wrapper_ty: #interp_crate::Interpretable<'__ir, __InterpI, __InterpL>
                            });
                        }
                    }
                    if predicates.is_empty() {
                        None
                    } else {
                        Some(syn::parse_quote! { where #(#predicates),* })
                    }
                })
                .validate(|ctx| {
                    // All variants must have #[wraps]
                    for stmt_ctx in ctx.statements.values() {
                        if !stmt_ctx.is_wrapper {
                            return Err(darling::Error::custom(format!(
                                "All variants must use #[wraps] for Interpretable derive. '{}' is missing #[wraps].",
                                stmt_ctx.stmt.name
                            )).with_span(&stmt_ctx.stmt.name));
                        }
                    }
                    Ok(())
                })
                .method(MethodSpec {
                    name: format_ident!("interpret"),
                    self_arg: quote! { &self },
                    params: vec![
                        quote! { interp: &mut __InterpI },
                        quote! { state: &'__ir __InterpL },
                    ],
                    return_type: Some(quote! {
                        #interp_crate::Continuation<__InterpI::Value, __InterpI::Ext>
                    }),
                    pattern: Box::new(DelegateToWrapper::new(
                        move |ctx: &DeriveContext<'_, StandardLayout>| {
                            let crate_path = ctx.meta.path_builder(&interp_crate).full_crate_path();
                            quote! { #crate_path::Interpretable<'__ir, __InterpI, __InterpL> }
                        },
                        format_ident!("interpret"),
                    ).require_all()),
                }),
        )
        .build()
}
```

Note: The `interp_crate` is moved into the closure for `DelegateToWrapper`. The exact trait path construction must match what the existing code produces — verify by comparing expanded output.

Important: `DelegateToWrapper` currently accesses `stmt_ctx.wrapper_type` and `stmt_ctx.wrapper_binding` from `StatementContext`. Verify these fields contain the right data for the interpreter case. The existing interpreter code accesses wrapper info from the `#[wraps]` attribute via `stmt.wraps` — the same data is pre-computed in `StatementContext.wrapper_type` and `StatementContext.wrapper_binding`.

**Step 2: Verify DelegateToWrapper generates correct delegation**

The existing code generates:
```rust
<WrappedType as Interpretable<'__ir, __InterpI, __InterpL>>::interpret(inner, interp, state)
```

`DelegateToWrapper` generates via `DelegationCall`:
```rust
<WrappedType as Trait>::method(field_binding)
```

Verify `DelegateToWrapper` passes the right arguments. The current `delegate.rs` passes `field` (the binding) but NOT extra args. Check if the `args` mechanism on `DelegateToWrapper` is needed, or if the delegation call format already handles this correctly.

**Important**: Look at `DelegationCall` in `tokens/delegation.rs` to see exactly what it generates. It may only pass the field binding as `self`, not the extra args (`interp`, `state`). If so, `DelegateToWrapper` may need enhancement, OR a `Custom` pattern may be needed for this specific case.

**Step 3: Build and test**

Run: `cargo build -p kirin-derive-interpreter && cargo nextest run -p kirin-interpreter`

**Step 4: Commit**

```
refactor(derive-interpreter): use TraitImplTemplate for Interpretable derive
```

---

### Task 6: Refactor CallSemantics derive to use TraitImplTemplate

**Files:**
- Rewrite: `crates/kirin-derive-interpreter/src/eval_call/mod.rs`
- Keep: `crates/kirin-derive-interpreter/src/eval_call/layout.rs` (unchanged)

**Step 1: Rewrite using TraitImplTemplate + SelectiveDelegation**

Similar pattern to Task 5 but with `EvalCallLayout` for `#[callable]` attribute parsing and `SelectiveDelegation` for conditional forwarding.

Key difference from Interpretable:
- Uses `Input::<EvalCallLayout>` (custom layout)
- Method has additional parameters: `stage`, `callee`, `args`
- Has an associated type `Result`
- `SelectiveDelegation` checks `#[callable]` attribute
- Fallback body is `Err(InterpreterError::MissingEntry)`
- Backward compat: if no `#[callable]` used anywhere, all wrappers forward

**Important**: `SelectiveDelegation` in `delegate.rs` already implements backward-compatible behavior (the `any_selected()` check). Verify it matches the existing `eval_call` logic.

**Step 2: Handle associated type `Result`**

The `CallSemantics` trait has `type Result`. Use `AssocTypeSpec::PerStatement` to compute it from the first wrapper type:
```rust
.assoc_type(AssocTypeSpec::PerStatement {
    name: format_ident!("Result"),
    compute: Box::new(|ctx, stmt_ctx| {
        let wrapper_ty = stmt_ctx.wrapper_type.unwrap();
        quote! { <#wrapper_ty as CallSemantics<...>>::Result }
    }),
})
```

**Step 3: Build and test**

Run: `cargo build -p kirin-derive-interpreter && cargo nextest run -p kirin-interpreter`

**Step 4: Commit**

```
refactor(derive-interpreter): use TraitImplTemplate for CallSemantics derive
```

---

### Task 7: Delete legacy code

Now that all consumers have been migrated, delete the legacy modules.

**Files:**
- Delete: `crates/kirin-derive-toolkit/src/scan.rs`
- Delete: `crates/kirin-derive-toolkit/src/emit.rs`
- Delete: `crates/kirin-derive-toolkit/src/generator.rs`
- Delete: `crates/kirin-derive-toolkit/src/generators/` (entire directory)
- Relocate: `crates/kirin-derive-toolkit/src/generators/stage_info.rs` → `crates/kirin-derive-toolkit/src/stage_info.rs`
- Modify: `crates/kirin-derive-toolkit/src/lib.rs`

**Step 1: Relocate stage_info.rs**

Copy `generators/stage_info.rs` to `src/stage_info.rs`. Update its imports:
- `use crate::stage::{self, StageVariantInfo};` stays the same (it only depends on `stage.rs`)

**Step 2: Delete legacy files**

Delete:
- `src/scan.rs`
- `src/emit.rs`
- `src/generator.rs`
- `src/generators/` (entire directory including builder/, field/, property/, common.rs, marker.rs, mod.rs)

**Step 3: Update `lib.rs`**

Remove module declarations:
```rust
// DELETE these:
pub mod emit;
pub mod generator;
pub mod generators;
pub mod scan;
```

Add:
```rust
pub mod stage_info;
```

Update prelude — remove legacy re-exports:
```rust
// DELETE from prelude:
pub use crate::emit::{self, Emit};
pub use crate::scan::{self, Scan};
```

**Step 4: Fix any remaining internal references**

The `template/mod.rs` imports `crate::generator::debug_dump`. This function needs to be relocated or inlined. Check what it does — likely a `KIRIN_DEBUG_DERIVE` env var check that prints the token stream. Move it to `misc.rs` or inline it in `template/mod.rs`.

**Step 5: Fix kirin-derive-ir import**

In `crates/kirin-derive-ir/src/lib.rs`, update:
```rust
// OLD:
use kirin_derive_toolkit::generators::stage_info::generate;
// NEW:
use kirin_derive_toolkit::stage_info::generate;
```

**Step 6: Build and test**

Run: `cargo build --workspace && cargo nextest run --workspace && cargo test --doc --workspace`

**Step 7: Commit**

```
refactor(derive-toolkit): delete legacy Scan/Emit/Generator code

Remove scan.rs, emit.rs, generator.rs, and entire generators/
directory. Relocate stage_info.rs to top-level module. Update
prelude to remove legacy re-exports.
```

---

### Task 8: Clean up prelude and update lib.rs docs

**Files:**
- Modify: `crates/kirin-derive-toolkit/src/lib.rs`

**Step 1: Simplify prelude**

The prelude should only export what downstream crates actually use:

```rust
pub mod prelude {
    // IR
    pub use crate::ir::{self, Layout, StandardLayout};
    pub use crate::ir::fields::{FieldCategory, FieldData, FieldInfo};

    // Context (now includes InputMeta, PathBuilder)
    pub use crate::context::{DeriveContext, InputMeta, PathBuilder, StatementContext};

    // Templates
    pub use crate::template::{
        self, BuilderTemplate, CompositeTemplate, FieldIterTemplateSet,
        MarkerTemplate, Template, TemplateBuilder, TraitImplTemplate,
        method_pattern::{self, AssocTypeSpec, Custom, MethodPattern, MethodSpec},
        trait_impl::{BoolPropertyConfig, FieldIterConfig},
    };

    // Codegen utilities
    pub use crate::codegen::{
        self, ConstructorBuilder, FieldBindings, GenericsBuilder,
        combine_where_clauses, deduplicate_types,
    };

    // Tokens
    pub use crate::tokens;

    // Hygiene
    pub use crate::hygiene::Hygiene;

    // External
    pub use darling;
    pub use proc_macro2;
}
```

**Step 2: Update module-level doc comment**

Update the `//!` doc at the top of `lib.rs` to remove references to "Legacy" layer (Scan, Emit, generators). The architecture table should only show IR, Templates, Tokens, and Support layers.

**Step 3: Build and verify**

Run: `cargo build --workspace`

**Step 4: Commit**

```
refactor(derive-toolkit): clean up prelude and update module docs
```

---

### Task 9: Final verification

**Step 1: Full build**

Run: `cargo build --workspace`

**Step 2: All tests**

Run: `cargo nextest run --workspace`

**Step 3: Doctests**

Run: `cargo test --doc --workspace`

**Step 4: Format**

Run: `cargo fmt --all`

**Step 5: Verify line count reduction**

Run: `git diff --stat` to confirm significant code reduction. Expected: ~2,000+ lines deleted.

**Step 6: Commit any formatting fixes**

```
chore: format
```
