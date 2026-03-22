# Derive Codegen Fixes

**Finding(s):** P0-1, P1-10, P1-11
**Wave:** 1
**Agent role:** Implementer
**Estimated effort:** moderate

---

## Issue

### P0-1: `has_signature` wrapper struct codegen generates unbound variable reference

**Crate:** kirin-derive-ir | **File:** `crates/kirin-derive-ir/src/has_signature.rs:18-28`

The `signature_body_struct` function for `#[wraps]` structs generates code referencing a field binding (e.g., `field_0`) without first destructuring `self`. The generated method body would be:

```rust
fn signature(&self) -> ... {
    <WrapperTy as HasSignature<WrapperTy>>::signature(field_0)
    //                                                ^^^^^^^ unbound
}
```

Compare with `bool_property.rs:183-189` which correctly emits `let Self #pattern = self;` before using the wrapper binding. The variant path (`signature_body_variant`) does not have this issue because the match arm already destructures.

### P1-10: `from_impl` drops wrapped value when variant has extra side-fields

**File:** `crates/kirin-derive-toolkit/src/template/builder_template/helpers.rs:559-587`

When a `#[wraps]` variant has additional fields beyond the wrapped field (e.g., `Foo { #[wraps] inner: InnerOp, extra: PhantomData<T> }`), the generated `From<InnerOp> for Enum` impl accepts `value: InnerOp` but never places it in the constructor. The `info.fields` only contains the non-wrapped extra fields, and the constructor is built solely from those fields (all initialized to defaults). The wrapped value itself is silently discarded.

**Why grouped:** Both are derive codegen bugs in the `#[wraps]` pattern, in closely related crates (kirin-derive-ir and kirin-derive-toolkit). Both require test-first approach per user decision.

**Crate(s):** kirin-derive-ir, kirin-derive-toolkit
**File(s):**
- `crates/kirin-derive-ir/src/has_signature.rs:18-28`
- `crates/kirin-derive-toolkit/src/template/builder_template/helpers.rs:530-590`
**Confidence:** confirmed (both)

## Guiding Principles

- "Derive Infrastructure Conventions" -- `#[wraps]` and `#[callable]` are intentionally separate from `#[kirin(...)]` for composability. The `DeriveContext` pre-computes `StatementContext` with wrapper_type, wrapper_binding, pattern.
- "No unsafe code." All implementations MUST use safe Rust.
- "less standalone function is better" -- prefer methods on existing types.
- P0-1 and P1-10: **Write tests triggering the bug BEFORE fixing** (user decision).

## Scope

**Files to modify:**
| File | Change Type | Description |
|------|-------------|-------------|
| `crates/kirin-derive-ir/src/has_signature.rs` | modify | Add `let Self #pattern = self;` destructuring in wrapper branch |
| `crates/kirin-derive-toolkit/src/template/builder_template/helpers.rs` | modify | Fix `from_impl` to include wrapper field in constructor for variants with extra fields; fix redundant condition |
| `crates/kirin-derive-ir/src/has_signature.rs` (tests) | modify | Add snapshot test for wrapper struct codegen |
| `crates/kirin-derive-toolkit/src/template/builder_template/` (tests) | modify | Add test for From impl with extra side-fields |

**Files explicitly out of scope:**
- `crates/kirin-derive-ir/src/bool_property.rs` -- reference only, already correct

## Verify Before Implementing

- [ ] **Verify: `signature_body_struct` wrapper branch still lacks destructuring**
  Run: Read `crates/kirin-derive-ir/src/has_signature.rs` lines 18-28
  Expected: No `let Self #pattern = self;` before the wrapper delegation call
  If this fails, STOP and report -- the bug may have been fixed since the review.

- [ ] **Verify: `bool_property.rs` pattern for reference**
  Run: Read `crates/kirin-derive-ir/src/bool_property.rs` lines 180-195
  Expected: See the `let Self #pattern = self;` destructuring pattern to replicate

- [ ] **Verify: `from_impl` else branch does not use `value`**
  Run: Read `crates/kirin-derive-toolkit/src/template/builder_template/helpers.rs` lines 555-590
  Expected: In the else branch (non-empty fields), the `ConstructorBuilder` only uses defaulted field names, not `value`
  If this fails, STOP and report.

- [ ] **Verify: existing tests compile and pass**
  Run: `cargo nextest run -p kirin-derive-ir && cargo nextest run -p kirin-derive-toolkit`
  Expected: All tests pass

## Regression Test (P0/P1 findings)

- [ ] **Write regression test for P0-1: unbound variable in wrapper struct HasSignature**
  Create a test that expands the derive for a wrapper struct with `#[wraps]` and verifies the generated code contains the destructuring `let Self ... = self;` line. This can be a snapshot test (using insta) or a compile-test that exercises the generated code.
  Test location: Inline `#[cfg(test)]` in `crates/kirin-derive-ir/src/has_signature.rs` or a companion test file.

- [ ] **Run the test -- confirm it fails (demonstrates the issue)**
  Run: `cargo nextest run -p kirin-derive-ir -E 'test(has_signature)'`
  Expected: FAIL -- the generated code references an unbound variable

- [ ] **Write regression test for P1-10: From impl drops wrapped value with extra fields**
  Create a test that generates a `From` impl for a `#[wraps]` variant with extra fields (e.g., a PhantomData field alongside the wrapped field). Verify the generated code includes `value` in the constructor. This can be a snapshot test comparing expected output.
  Test location: Inline `#[cfg(test)]` in `crates/kirin-derive-toolkit/src/template/builder_template/` or existing test module.

- [ ] **Run the test -- confirm it fails (demonstrates the issue)**
  Run: `cargo nextest run -p kirin-derive-toolkit -E 'test(from_impl)'`
  Expected: FAIL -- the generated From impl does not include the wrapped value

## Implementation Steps

- [ ] **Step 1: Fix P0-1 -- Add destructuring to `signature_body_struct` wrapper branch**
  In `crates/kirin-derive-ir/src/has_signature.rs`, modify the `if stmt_ctx.is_wrapper` branch to add:
  ```rust
  let pattern = &stmt_ctx.pattern;
  ```
  and wrap the return body with:
  ```rust
  return Ok(quote! {
      let Self #pattern = self;
      <#wrapper_ty as #full_trait_path<#wrapper_ty>>::#trait_method(#field)
  });
  ```
  This mirrors the pattern in `bool_property.rs:183-189`.

- [ ] **Step 2: Run P0-1 regression test -- confirm it passes**
  Run: `cargo nextest run -p kirin-derive-ir -E 'test(has_signature)'`
  Expected: PASS

- [ ] **Step 3: Fix P1-10 -- Include wrapped value in From impl for variants with extra fields**
  In `crates/kirin-derive-toolkit/src/template/builder_template/helpers.rs`, in the `from_impl` function's else branch (non-empty `info.fields`), modify the `ConstructorBuilder`'s field-value closure to include the wrapper field. The wrapper field name can be obtained from the `StatementInfo`'s wrapper metadata. The `value` parameter must be mapped to the wrapper field name in the constructor.

- [ ] **Step 4: Fix P1-11 -- Remove redundant condition**
  In the same function, in the `if info.fields.is_empty()` branch, change:
  ```rust
  let initialization = if is_tuple || info.fields.is_empty() {
  ```
  to:
  ```rust
  let initialization = if is_tuple {
  ```

- [ ] **Step 5: Run P1-10 regression test -- confirm it passes**
  Run: `cargo nextest run -p kirin-derive-toolkit -E 'test(from_impl)'`
  Expected: PASS

- [ ] **Step 6: Run full workspace tests**
  Run: `cargo nextest run --workspace`
  Expected: All tests pass

- [ ] **Step 7: Run clippy**
  Run: `cargo clippy -p kirin-derive-ir && cargo clippy -p kirin-derive-toolkit`
  Expected: No warnings

## Must Not Do

- Do NOT introduce `#[allow(...)]` annotations to suppress warnings -- fix the underlying cause.
- Do NOT leave clippy warnings. Run `cargo clippy -p <crate>` before reporting completion and fix all warnings.
- Do NOT change the `StatementContext` API -- only use existing fields (`is_wrapper`, `wrapper_type`, `wrapper_binding`, `pattern`).
- Do NOT modify `bool_property.rs` -- it is the reference implementation, not a target.
- No unsafe code (AGENTS.md: all implementations MUST use safe Rust).

## Validation

**Per-step checks:**
- After step 1: `cargo check -p kirin-derive-ir` -- Expected: compiles
- After step 2: `cargo nextest run -p kirin-derive-ir -E 'test(has_signature)'` -- Expected: PASS
- After step 3-4: `cargo check -p kirin-derive-toolkit` -- Expected: compiles
- After step 5: `cargo nextest run -p kirin-derive-toolkit -E 'test(from_impl)'` -- Expected: PASS

**Final checks:**
```bash
cargo clippy -p kirin-derive-ir              # Expected: no warnings
cargo clippy -p kirin-derive-toolkit         # Expected: no warnings
cargo nextest run -p kirin-derive-ir         # Expected: all tests pass
cargo nextest run -p kirin-derive-toolkit    # Expected: all tests pass
cargo nextest run --workspace                # Expected: no regressions
cargo test --doc -p kirin-derive-ir          # Expected: all doctests pass
```

**Snapshot tests:** Yes -- if snapshot tests exist in these crates, run `cargo insta test -p kirin-derive-ir` and `cargo insta test -p kirin-derive-toolkit` and report changes. Do NOT auto-accept.

## Success Criteria

1. P0-1: Generated `HasSignature` code for wrapper structs correctly destructures `self` before referencing fields -- verified by a test that compiles and runs the generated code.
2. P1-10: Generated `From` impls for wrapper variants with extra fields correctly include the wrapped value in the constructor -- verified by a test or snapshot.
3. P1-11: The redundant condition is removed and the code still generates correct syntax for both tuple and named-field wrappers.
4. No regressions in any workspace test.

**Is this a workaround or a real fix?**
This is the real fix. Both bugs are in code generation: the generated code is incorrect. The fix makes the generated code correct by including the missing destructuring (P0-1) and the missing value (P1-10).
