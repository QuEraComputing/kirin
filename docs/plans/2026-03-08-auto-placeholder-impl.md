# Auto-Placeholder Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Make the derive system auto-infer `T::placeholder()` for unannotated ResultValue fields, eliminating manual `#[kirin(type = T::placeholder())]` annotations and `+ Placeholder` bounds from dialect author code.

**Architecture:** Thread the enum-level `ir_type` path into field parsing so unannotated ResultValue fields get a default `ir_type::placeholder()` expression. Track auto-generated defaults with a flag in `FieldData::Result`. Use that flag in builder and EmitIR codegen to conditionally add `Placeholder` bounds.

**Tech Stack:** Rust proc-macro infrastructure (syn, quote, darling), kirin-derive-toolkit, kirin-derive-chumsky

---

### Task 1: Add `is_auto_placeholder` flag to `FieldData::Result`

**Files:**
- Modify: `crates/kirin-derive-toolkit/src/ir/fields/data.rs`

**Step 1: Add the flag**

Change `FieldData::Result` to include the tracking flag:

```rust
Result {
    ssa_type: syn::Expr,
    is_auto_placeholder: bool,
},
```

**Step 2: Update the `Clone` impl**

In the manual `Clone` impl, clone the new field:

```rust
FieldData::Result { ssa_type, is_auto_placeholder } => FieldData::Result {
    ssa_type: ssa_type.clone(),
    is_auto_placeholder: *is_auto_placeholder,
},
```

**Step 3: Update test helpers in `info.rs`**

In `crates/kirin-derive-toolkit/src/ir/fields/info.rs`, update `make_result_field` and any other test constructors to include `is_auto_placeholder: false`.

Also update any `FieldData::Result { ssa_type: ... }` patterns in `kind_name_all_categories` test.

**Step 4: Add accessor to `FieldInfo`**

In `crates/kirin-derive-toolkit/src/ir/fields/info.rs`, add:

```rust
/// Return `true` if this Result field's type was auto-generated from `ir_type::placeholder()`.
pub fn is_auto_placeholder(&self) -> bool {
    matches!(&self.data, FieldData::Result { is_auto_placeholder: true, .. })
}
```

**Step 5: Run tests**

Run: `cargo nextest run -p kirin-derive-toolkit`
Expected: PASS

**Step 6: Commit**

```
feat(derive-toolkit): add is_auto_placeholder flag to FieldData::Result
```

---

### Task 2: Thread `ir_type` into field parsing and default ResultValue

**Files:**
- Modify: `crates/kirin-derive-toolkit/src/ir/statement/definition.rs`
- Modify: `crates/kirin-derive-toolkit/src/ir/input.rs`

**Step 1: Add `ir_type` parameter to `parse_field`**

In `definition.rs`, change `parse_field` signature:

```rust
fn parse_field(index: usize, f: &syn::Field, ir_type: &syn::Path) -> darling::Result<FieldInfo<L>> {
```

In the ResultValue branch (lines 173-186), instead of erroring on missing `ssa_ty`, generate the default:

```rust
if let Some(collection) = Collection::from_type(ty, "ResultValue") {
    let (ssa_type, is_auto_placeholder) = match kirin_opts.ssa_ty {
        Some(expr) => (expr, false),
        None => (syn::parse_quote!(#ir_type::placeholder()), true),
    };
    return Ok(FieldInfo {
        index,
        ident,
        collection,
        data: FieldData::Result { ssa_type, is_auto_placeholder },
    });
}
```

**Step 2: Thread `ir_type` through `update_fields`**

Change `update_fields` signature:

```rust
fn update_fields(mut self, wraps: bool, fields: &syn::Fields, ir_type: &syn::Path) -> darling::Result<Self> {
```

Pass `ir_type` to all `Self::parse_field(i, f, ir_type)` calls.

**Step 3: Thread `ir_type` through `from_derive_input` and `from_variant`**

Change `Statement::from_derive_input` to accept `ir_type`:

```rust
pub fn from_derive_input(input: &syn::DeriveInput, ir_type: &syn::Path) -> darling::Result<Self> {
```

Pass `ir_type` to `update_fields`.

Change `Statement::from_variant` to accept `ir_type`:

```rust
pub fn from_variant(wraps: bool, variant: &syn::Variant, ir_type: &syn::Path) -> darling::Result<Self> {
```

Pass `ir_type` to `update_fields`.

**Step 4: Update `Input::from_derive_input` to parse attrs first**

In `input.rs`, restructure to parse `GlobalOptions` before creating statements:

```rust
pub fn from_derive_input(input: &syn::DeriveInput) -> darling::Result<Self> {
    let attrs = GlobalOptions::from_derive_input(input)?;
    let extra_attrs = L::ExtraGlobalAttrs::from_derive_input(input)?;
    let ir_type = &attrs.ir_type;

    let data = match &input.data {
        syn::Data::Struct(_) => {
            Data::Struct(DataStruct(Statement::from_derive_input(input, ir_type)?))
        }
        syn::Data::Enum(data) => Data::Enum(DataEnum {
            variants: data
                .variants
                .iter()
                .map(|v| {
                    Statement::from_variant(
                        input.attrs.iter().any(|f| f.path().is_ident("wraps")),
                        v,
                        ir_type,
                    )
                })
                .collect::<darling::Result<Vec<_>>>()?,
        }),
        syn::Data::Union(_) => {
            return Err(darling::Error::custom(
                "Kirin ASTs can only be derived for structs or enums",
            )
            .with_span(input));
        }
    };

    Ok(Self {
        name: input.ident.clone(),
        generics: input.generics.clone(),
        attrs,
        extra_attrs,
        data,
        raw_attrs: input.attrs.clone(),
    })
}
```

**Step 5: Update existing `FieldData::Result` construction sites**

Search for any other places that construct `FieldData::Result { ssa_type: ... }` and add `is_auto_placeholder: false` (these are explicit annotations). The test helpers in `info.rs` were already updated in Task 1.

**Step 6: Run tests**

Run: `cargo nextest run -p kirin-derive-toolkit`
Expected: PASS (no behavior change yet — all existing dialects have explicit annotations)

**Step 7: Commit**

```
feat(derive-toolkit): thread ir_type into field parsing for auto-placeholder defaults
```

---

### Task 3: Conditional Placeholder bound in builder template

**Files:**
- Modify: `crates/kirin-derive-toolkit/src/template/builder_template/helpers.rs`

**Step 1: Detect auto-placeholder fields in `build_fn_for_statement`**

In `build_fn_for_statement` (line 286), after computing the result names, check if any ResultValue field uses auto-placeholder:

```rust
let needs_placeholder_bound = info
    .fields
    .iter()
    .any(|f| f.is_auto_placeholder());
```

**Step 2: Conditionally add the Placeholder bound**

In the `fn_tokens` generation (lines 313-319), add the Placeholder bound when needed:

```rust
let placeholder_bound = if needs_placeholder_bound {
    quote! { , #ir_type: #crate_path::Placeholder }
} else {
    quote! {}
};

let fn_tokens = quote! {
    pub fn #build_fn_name<Lang>(stage: &mut #crate_path::StageInfo<Lang>, #(#inputs),*) -> #build_result_path
    where
        Lang: #crate_path::Dialect + From<#self_ty>,
        Lang::Type: From<#ir_type>
        #placeholder_bound
    #body
};
```

**Step 3: Run tests**

Run: `cargo nextest run -p kirin-derive-toolkit`
Expected: PASS

**Step 4: Commit**

```
feat(derive-toolkit): conditionally add Placeholder bound in builder when auto-placeholder used
```

---

### Task 4: Make EmitIR Placeholder bound conditional

**Files:**
- Modify: `crates/kirin-derive-chumsky/src/codegen/emit_ir/generate.rs`
- Modify: `crates/kirin-derive-chumsky/src/codegen/emit_ir/self_emit.rs`

**Step 1: Add helper to check if any field uses auto-placeholder**

In `generate.rs`, add a method to `GenerateEmitIR`:

```rust
fn needs_placeholder_bound(
    &self,
    ir_input: &kirin_derive_toolkit::ir::Input<ChumskyLayout>,
) -> bool {
    match &ir_input.data {
        kirin_derive_toolkit::ir::Data::Struct(data) => {
            data.0.fields.iter().any(|f| f.is_auto_placeholder())
        }
        kirin_derive_toolkit::ir::Data::Enum(data) => data
            .variants
            .iter()
            .any(|stmt| stmt.fields.iter().any(|f| f.is_auto_placeholder())),
    }
}
```

**Step 2: Make Placeholder bound conditional in `generate_emit_impl`**

In `generate_emit_impl` (line 229), replace the unconditional Placeholder bound:

```rust
let placeholder_bound = if self.needs_placeholder_bound(ir_input) {
    quote! { <Language as #ir_path::Dialect>::Type: #ir_path::Placeholder, }
} else {
    quote! {}
};
```

Then use `#placeholder_bound` in `base_bounds` instead of the hardcoded line.

**Step 3: Same change in `self_emit.rs`**

In `generate_ast_self_emit_impl` (line 95), make the same conditional change. The `needs_placeholder_bound` method is on `GenerateEmitIR` so it's accessible.

**Step 4: Update snapshot tests**

Run: `cargo nextest run -p kirin-derive-chumsky`
Then: `cargo insta review` to accept updated snapshots (the Placeholder bound may disappear from snapshots that use non-generic types without ResultValue auto-placeholder).

**Step 5: Commit**

```
feat(derive-chumsky): make EmitIR Placeholder bound conditional on auto-placeholder fields
```

---

### Task 5: Migrate `kirin-arith` — remove Placeholder boilerplate

**Files:**
- Modify: `crates/kirin-arith/src/lib.rs`
- Modify: `crates/kirin-arith/src/interpret_impl.rs`

**Step 1: Remove `+ Placeholder` from enum definition**

In `lib.rs`, change:
```rust
pub enum Arith<T: CompileTimeValue + Placeholder> {
```
to:
```rust
pub enum Arith<T: CompileTimeValue> {
```

**Step 2: Remove all `#[kirin(type = T::placeholder())]` annotations**

Remove from all 6 variants: Add, Sub, Mul, Div, Rem, Neg.

**Step 3: Remove `+ Placeholder` from interpret_impl.rs**

Change line 31:
```rust
T: CompileTimeValue + Placeholder,
```
to:
```rust
T: CompileTimeValue,
```

Remove the `Placeholder` import if no longer used.

**Step 4: Run tests**

Run: `cargo nextest run -p kirin-arith`
Expected: PASS

**Step 5: Commit**

```
refactor(arith): remove manual Placeholder boilerplate (auto-inferred by derive)
```

---

### Task 6: Migrate `kirin-bitwise` — remove Placeholder boilerplate

**Files:**
- Modify: `crates/kirin-bitwise/src/lib.rs`
- Modify: `crates/kirin-bitwise/src/interpret_impl.rs`
- Modify: `crates/kirin-bitwise/src/tests.rs`

Same pattern as Task 5:
1. Remove `+ Placeholder` from `Bitwise<T>` enum definition
2. Remove all `#[kirin(type = T::placeholder())]` from 6 variants
3. Remove `+ Placeholder` from interpret_impl bounds and unused import
4. Remove `impl Placeholder for UnitTy` from tests.rs (and the import if any)

Run: `cargo nextest run -p kirin-bitwise`
Expected: PASS

Commit: `refactor(bitwise): remove manual Placeholder boilerplate (auto-inferred by derive)`

---

### Task 7: Migrate `kirin-cmp` — remove Placeholder boilerplate

**Files:**
- Modify: `crates/kirin-cmp/src/lib.rs`
- Modify: `crates/kirin-cmp/src/interpret_impl.rs`
- Modify: `crates/kirin-cmp/src/tests.rs`

Same pattern:
1. Remove `+ Placeholder` from `Cmp<T>` enum definition
2. Remove all `#[kirin(type = T::placeholder())]` from 6 variants
3. Remove `+ Placeholder` from interpret_impl bounds and unused import
4. Remove `impl Placeholder for UnitTy` from tests.rs

Run: `cargo nextest run -p kirin-cmp`
Expected: PASS

Commit: `refactor(cmp): remove manual Placeholder boilerplate (auto-inferred by derive)`

---

### Task 8: Migrate `kirin-function` — remove Placeholder boilerplate

**Files:**
- Modify: `crates/kirin-function/src/lib.rs`
- Modify: `crates/kirin-function/src/call.rs`
- Modify: `crates/kirin-function/src/bind.rs`
- Modify: `crates/kirin-function/src/lambda.rs`
- Modify: `crates/kirin-function/src/interpret_impl.rs`

**Step 1: Remove `+ Placeholder` from Lexical and Lifted enums** in `lib.rs`

**Step 2: Remove from Call struct** in `call.rs`:
- Remove `+ Placeholder` from struct definition and `impl` block
- Remove `#[kirin(type = T::placeholder())]` from `res` field
- Remove `impl Placeholder for UnitTy` from tests module

**Step 3: Remove from Bind** in `bind.rs`:
- Remove `+ Placeholder` from struct definition
- Remove `#[kirin(type = T::placeholder())]` from `res` field

**Step 4: Remove from Lambda** in `lambda.rs`:
- Remove `+ Placeholder` from struct definition
- Remove `#[kirin(type = T::placeholder())]` from `res` field

**Step 5: Remove from interpret_impl.rs**:
- Remove `+ kirin::prelude::Placeholder` from all 4 trait impl where clauses
- Remove unused `Placeholder` import if applicable

Note: `FunctionBody` and `Return` don't have ResultValue fields — no changes needed.

Run: `cargo nextest run -p kirin-function`
Expected: PASS

Commit: `refactor(function): remove manual Placeholder boilerplate (auto-inferred by derive)`

---

### Task 9: Migrate `kirin-interpreter` stage_dispatch test

**Files:**
- Modify: `crates/kirin-interpreter/tests/stage_dispatch.rs`

Remove `+ Placeholder` from `StageCall<T>` struct definition, impl block, and trait impl where clause. Remove `#[kirin(type = T::placeholder())]` from `result` field.

Run: `cargo nextest run -p kirin-interpreter`
Expected: PASS

Commit: `refactor(interpreter): remove Placeholder boilerplate from stage_dispatch test`

---

### Task 10: Full workspace build and test

Run: `cargo build --workspace`
Then: `cargo nextest run --workspace`
Then: `cargo test --doc --workspace`

Fix any remaining compilation errors from the migration.

Review and accept any updated snapshots: `cargo insta review`

Commit: `fix: resolve remaining auto-placeholder migration issues`

---

### Task 11: Update AGENTS.md and memory

Update AGENTS.md if needed — document the auto-placeholder convention:
- ResultValue fields without `#[kirin(type = ...)]` auto-default to `ir_type::placeholder()`
- Explicit `#[kirin(type = expr)]` overrides the default
- `Placeholder` bound only appears in derive-generated code

Update memory file with the new convention.

Commit: `docs: document auto-placeholder convention for ResultValue fields`
