# U3: Derive Infrastructure Review Report

**Date:** 2026-03-22
**Scope:** 5 crates (~16,172 lines): kirin-derive-toolkit, kirin-derive-ir, kirin-derive-chumsky, kirin-derive-interpreter, kirin-derive-prettyless
**Reviewer Perspectives:** Formalism, Code Quality, Ergonomics/DX, Soundness Adversary, Dialect Author, Compiler Engineer

---

## High Priority (P0-P1)

### [P0] [confirmed] `has_signature` wrapper struct path generates unbound variable reference

**File:** `crates/kirin-derive-ir/src/has_signature.rs:18-28`
**Perspective:** Soundness Adversary

The `signature_body_struct` function, when the struct is a wrapper (`#[wraps]`), generates code that references the wrapper field binding (e.g., `field_0`) without first destructuring `self`. The generated method body would be:

```rust
fn signature(&self) -> ... {
    <WrapperTy as HasSignature<WrapperTy>>::signature(field_0)
    //                                                ^^^^^^^ unbound
}
```

Compare with `bool_property.rs:183-189` which correctly emits `let Self #pattern = self;` before using the wrapper binding. The variant path (`signature_body_variant`) does not have this issue because the match arm already destructures.

**Suggested action:** Add a destructuring `let` before the delegation call in the wrapper branch, mirroring `BoolProperty::for_struct`:

```rust
if stmt_ctx.is_wrapper {
    let pattern = &stmt_ctx.pattern;
    // ...
    return Ok(quote! {
        let Self #pattern = self;
        <#wrapper_ty as #full_trait_path<#wrapper_ty>>::#trait_method(#field)
    });
}
```

---

### [P1] [confirmed] `from_impl` drops the wrapped value when wrapper variant has extra side-fields

**File:** `crates/kirin-derive-toolkit/src/template/builder_template/helpers.rs:559-587`
**Perspective:** Soundness Adversary

When a `#[wraps]` variant has additional fields beyond the wrapped field (e.g., `Foo { #[wraps] inner: InnerOp, extra: PhantomData<T> }`), the generated `From<InnerOp> for Enum` impl accepts `value: InnerOp` but never uses `value` in the constructor. The `info.fields` only contains the non-wrapped extra fields, and the constructor is built solely from those fields (all initialized to defaults). The wrapped value itself is silently discarded.

The `fields.is_empty()` branch (line 544) correctly places `value` as the constructor argument, but the `else` branch does not.

**Suggested action:** In the `else` branch, incorporate the wrapper field (`value`) into the constructor alongside the defaulted side-fields. The constructor needs to set the wrapped field to `value` and the extra fields to their defaults.

---

### [P1] [confirmed] Redundant always-true condition in `from_impl`

**File:** `crates/kirin-derive-toolkit/src/template/builder_template/helpers.rs:545`
**Perspective:** Code Quality

```rust
if info.fields.is_empty() {
    let initialization = if is_tuple || info.fields.is_empty() {
```

The outer `if` already guarantees `info.fields.is_empty()` is true, making the inner `|| info.fields.is_empty()` redundant. The condition always takes the `is_tuple`-style path regardless of `is_tuple`. For a named-field wrapper with no extra fields (e.g., `Foo { inner: InnerOp }`), this would generate tuple syntax `(value)` instead of brace syntax `{ inner: value }`.

However, this only fires for wrapper variants (which always have the wrapped field removed from `fields`), and single-field wrappers with named fields are rare. The practical impact is that the generated code would still compile if the IR type accepts tuple construction, but it could produce incorrect code for named-field structs.

**Suggested action:** Remove the redundant condition. Inspect whether the `is_tuple` path or brace path is correct for named single-field wrappers.

---

## Medium Priority (P2)

### [P2] [confirmed] Duplicated `is_call_forwarding` and `collect_callable_wrappers` across interpreter derive modules

**File:** `crates/kirin-derive-interpreter/src/eval_call/generate.rs:154-170` and `crates/kirin-derive-interpreter/src/ssa_cfg_region/generate.rs:106-122`
**Perspective:** Code Quality

Both `eval_call::generate` and `ssa_cfg_region::generate` define identical `is_call_forwarding` and `collect_callable_wrappers` functions (differing only in doc comments). Both operate on `DeriveContext<'_, EvalCallLayout>` and perform the same callable/wrapper filtering logic.

**Suggested action:** Extract these two functions into a shared module within `kirin-derive-interpreter` (e.g., `src/common.rs` or directly in `src/eval_call/mod.rs` and re-exported). The SSACFGRegion module already imports `EvalCallLayout` from `eval_call`.

---

### [P2] [confirmed] `DeriveContext` has a no-op `ToTokens` impl

**File:** `crates/kirin-derive-toolkit/src/context/mod.rs:59-61`
**Perspective:** Code Quality / Formalism

```rust
impl<L: Layout> ToTokens for DeriveContext<'_, L> {
    fn to_tokens(&self, _tokens: &mut TokenStream) {}
}
```

This generates zero tokens. If anything interpolates `#ctx` in a `quote!` block, it silently produces nothing rather than failing at compile time. This is a potential pit: a derive author might accidentally interpolate the context expecting output and get no error.

**Suggested action:** Remove the `ToTokens` impl if unused, or document why it exists (e.g., satisfying a trait bound somewhere). If it serves as a deliberate "produces nothing" marker, add a doc comment explaining the intent.

---

### [P2] [confirmed] `to_snake_case` does not handle consecutive uppercase correctly

**File:** `crates/kirin-derive-toolkit/src/misc.rs:43-59`
**Perspective:** Compiler Engineer

`to_snake_case("HTTPParser")` produces `"h_t_t_p_parser"` rather than the conventional `"http_parser"`. While the existing tests document this behavior (`to_snake_case("ABC")` == `"a_b_c"`), this function is used for builder function name generation (`build_fn_name`) and result module naming, so an acronym-containing type name like `HTTPGet` would produce `op_h_t_t_p_get` as the builder function name.

**Suggested action:** If acronym-preserving snake_case is desired, implement the standard algorithm (insert underscore before an uppercase letter only when followed by a lowercase letter or preceded by a lowercase letter). Otherwise, document the limitation prominently.

---

### [P2] [confirmed] `InherentImplTemplate` and `TypeDefTemplate` are structurally identical

**File:** `crates/kirin-derive-toolkit/src/template/inherent_impl.rs` and `crates/kirin-derive-toolkit/src/template/type_def.rs`
**Perspective:** Code Quality / Formalism

Both types are thin wrappers around `Box<dyn Fn(&DeriveContext<'_, L>) -> darling::Result<Vec<TokenStream>>>` with identical `new()` and `emit()` implementations. They are semantically distinct (one generates inherent impls, the other type definitions), but the code is 100% duplicated.

**Suggested action:** Either (a) merge into a single generic "closure template" type with a semantic marker, or (b) accept the duplication as intentional for naming clarity and add a comment. Given the small size (~30 lines each), the duplication is tolerable but worth noting.

---

### [P2] [confirmed] `is_type` matches on last path segment only, risking false positives

**File:** `crates/kirin-derive-toolkit/src/misc.rs:62-73`
**Perspective:** Soundness Adversary

`is_type` checks only the last segment of a type path. A user type `my_module::SSAValue` or `my_crate::Block` would be classified as an IR primitive. In practice this is unlikely to cause issues since Kirin IR types are imported directly, but it is a latent correctness risk.

**Suggested action:** Document the assumption that IR primitive types are always imported unqualified. Alternatively, check the full path length is 1 (bare ident) before matching, or match on the known set of segments.

---

### [P2] [likely] `BuilderPattern` implements `MethodPattern` but always returns errors

**File:** `crates/kirin-derive-toolkit/src/template/method_pattern/builder_pattern.rs:1-35`
**Perspective:** Ergonomics/DX

`BuilderPattern` exists "for API completeness" but both `for_struct` and `for_variant` unconditionally return errors. If a derive author accidentally uses it as a `MethodPattern`, they get a runtime error message rather than a compile-time type error. This is a concept-budget concern: the type exists in the public API but is never usable.

**Suggested action:** Consider removing `BuilderPattern` from the public API, or make it `pub(crate)`. If it must exist, add a deprecation or doc comment warning that it is not a functional `MethodPattern`.

---

### [P2] [confirmed] `FieldData` manual `Clone` impl could be derived

**File:** `crates/kirin-derive-toolkit/src/ir/fields/data.rs:70-103`
**Perspective:** Code Quality

`FieldData<L>` has a manual `Clone` implementation that does the same thing as `#[derive(Clone)]` would generate. The manual impl is 33 lines that add maintenance burden without providing any customization. The same applies to `FieldInfo<L>` in `info.rs:31-39`.

The likely reason is that `L: Layout` doesn't provide `Clone` bounds on all associated types automatically, but since `Layout` requires `FromDeriveInput + FromField + Clone + Debug` on all its associated types, derive should work.

**Suggested action:** Try replacing the manual `Clone` impls with `#[derive(Clone)]` and verify it compiles. The `Layout` bounds should be sufficient.

---

## Low Priority (P3)

### [P3] [confirmed] `DEFAULT_INTERP_CRATE` uses `::kirin_interpreter` (not `::kirin::interpreter`)

**File:** `crates/kirin-derive-interpreter/src/interpretable.rs:9` and `crates/kirin-derive-interpreter/src/eval_call/generate.rs:12`
**Perspective:** Dialect Author

The default interpreter crate path is `::kirin_interpreter` rather than following the `::kirin::` prefix pattern used by IR (`::kirin::ir`) and parsers (`::kirin::parsers`). This is likely correct for the crate's re-export structure, but a dialect author might expect consistency. If `kirin` re-exports `kirin_interpreter` under a different path, this would need updating.

**Suggested action:** Verify that `::kirin_interpreter` is the correct public path for downstream users. If `kirin` provides a re-export (e.g., `::kirin::interp`), update the default.

---

### [P3] [confirmed] `error_unknown_attribute` uses hardcoded attribute lists

**File:** `crates/kirin-derive-toolkit/src/misc.rs:131-173`
**Perspective:** Ergonomics/DX

The unknown-attribute error handler maintains hardcoded lists of valid attribute names across different levels (type, statement, field). If new attributes are added, this function must be manually updated to provide accurate error messages. The current lists may already be stale (e.g., `"text"` is listed as a per-statement attribute but doesn't appear in `StatementOptions`).

**Suggested action:** Remove `"text"` from the per-statement list if it is not a valid attribute. Consider generating the attribute lists from the `darling` struct definitions or at least colocating them with the struct definitions.

---

### [P3] [confirmed] `from_str` panics on invalid input

**File:** `crates/kirin-derive-toolkit/src/misc.rs:20-22`
**Perspective:** Compiler Engineer

```rust
pub fn from_str<T: syn::parse::Parse>(s: impl Into<String>) -> T {
    syn::parse_str(&s.into()).unwrap()
}
```

This `unwrap()` will panic in a proc-macro context, producing an ICE-style error rather than a user-friendly diagnostic. All call sites pass string literals (e.g., `from_str("IsPure")`), so the panic is unreachable in practice. However, the function is `pub` and available for use by downstream derive authors.

**Suggested action:** Either make the function `pub(crate)` to restrict its use to controlled call sites, or change the return type to `Result` / add a doc comment warning about the panic.

---

### [P3] [confirmed] `has_signature` struct non-wrapper assumes Signature field is named

**File:** `crates/kirin-derive-ir/src/has_signature.rs:39`
**Perspective:** Soundness Adversary

```rust
let field_ident = field.ident.as_ref().expect("Signature field must be named");
```

This panics for tuple structs with a `Signature` field (e.g., `struct MyOp(Signature<T>)`). While it is unlikely someone would use a tuple struct with a Signature field, the `expect` message is misleading.

**Suggested action:** Handle the `None` case gracefully, either by generating positional access (`self.0`) or by returning an error.

---

### [P3] [confirmed] `ChumskyFieldAttrs` is currently empty

**File:** `crates/kirin-derive-chumsky/src/attrs.rs:33-37`
**Perspective:** Code Quality

```rust
pub struct ChumskyFieldAttrs {
    // Currently no field-level chumsky attributes
}
```

This struct exists purely as a placeholder. It occupies a slot in the `Layout` type and is instantiated for every field parsed by the chumsky layout. The comment indicates awareness. No action needed unless the placeholder adds measurable compile-time cost.

**Suggested action:** Informational only. Keep as-is for future extensibility.

---

## Strengths

1. **Template system is well-designed.** The `Input::compose().add(template).build()` pipeline is clean and composable. The three-layer approach (declarative factory methods, composition with MethodPattern, closure-based Custom) provides good ergonomics at each level of complexity.

2. **Layout extensibility.** The `Layout` trait with associated types for per-level custom attributes is elegant. `StandardLayout` provides the zero-cost default, while `ChumskyLayout` and `EvalCallLayout` demonstrate real extensions. The `extra_statement_attrs_from_input` hook with lenient parsing is a well-thought-out solution to shared namespaces.

3. **Comprehensive validation.** Format string validation in `kirin-derive-chumsky` is thorough: checks for missing fields, duplicate default occurrences, SSA name requirements, body projection completeness, and legacy patterns. The error messages are actionable and specific.

4. **Excellent test coverage.** All 5 crates have inline unit tests, snapshot tests for generated code, and edge-case tests (empty enums, missing attributes, wrong attribute levels). The derive-interpreter crate has tests for every derive variant (enum, struct, wraps, non-wraps, generics).

5. **Clean separation of concerns.** The toolkit's IR/template/tokens/codegen layering is well-separated. Each layer has a clear responsibility, and the dependency flow is strictly downward.

6. **Good error reporting via darling.** Error accumulation (`darling::Error::accumulator()`) is used consistently, allowing multiple errors to surface at once rather than stopping at the first.

7. **DeriveContext pre-computation.** Building `StatementContext` once and sharing it across templates avoids redundant work. The `IndexMap` preserves declaration order, which matters for deterministic codegen.

8. **Hygiene module.** The `Hygiene` type with `__` prefix generation is a practical solution to proc-macro name collision, and it's clean and simple.

---

## Filtered Findings (Intentional Design, Not Flagged)

- Darling re-export pattern (`kirin_derive_toolkit::prelude::darling`) and kirin-derive-chumsky's direct darling dep.
- `#[wraps]` and `#[callable]` parsed manually via `attrs.iter().any()`.
- `#[kirin(...)]` path syntax (`crate = kirin_ir` not `crate = "kirin_ir"`).
- Custom Layout for derive-specific attributes (EvalCallLayout, ChumskyLayout).
- Global-only fields in shared namespaces using lenient intermediate struct.
- HasCratePath on ExtraGlobalAttrs with `Input::extra_crate_path()`.
- Auto-placeholder for ResultValue fields.
- Legacy Scan/Emit traits in kirin-derive-chumsky (not yet migrated to templates).
- `#[allow(clippy::type_complexity)]` at crate level for derive-toolkit (complex closures are inherent to template system).
- `#[allow(clippy::too_many_arguments)]` at crate level for derive-chumsky (codegen parameter threading).
- `#[allow(clippy::large_enum_variant)]` on `Data<L>` and `FieldData<L>` (proc-macro code, not hot path).
