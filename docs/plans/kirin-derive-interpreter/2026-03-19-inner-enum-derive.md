# Extend `#[derive(Interpretable)]` for Enum-Level `#[wraps]`

## Problem

`#[derive(Interpretable)]` currently only works with per-variant `#[wraps]`. Enums with enum-level `#[wraps]` (where the attribute is on the enum itself rather than each variant) fail validation with:

> Cannot derive `Interpretable`: variant(s) X, Y are not `#[wraps]`.

This forces manual `Interpretable` impls for enums like `Lexical<T>`, `Lifted<T>`, and `StructuredControlFlow<T>`, which are ~15-20 lines of pure delegation boilerplate each.

### Per-variant vs enum-level `#[wraps]`

Per-variant (works today):
```rust
enum MyLang {
    #[wraps]
    Arith(ArithOps),
    #[wraps]
    Cf(CfOps),
}
```

Enum-level (does NOT work today):
```rust
#[wraps]
enum Lexical<T: CompileTimeValue> {
    FunctionBody(FunctionBody<T>),
    Lambda(Lambda<T>),
    Call(Call<T>),
    Return(Return<T>),
}
```

With enum-level `#[wraps]`, every variant implicitly delegates to its inner type. The `#[derive(Dialect, HasParser, PrettyPrint)]` macros already handle this correctly. Only `Interpretable` and `SSACFGRegion` lag behind.

## Research Findings

### Current derive validation (interpretable.rs:52-67)

The validation closure checks `ctx.statements.values().filter(|s| !s.is_wrapper)` and rejects any non-wrapper variants. The issue is that `is_wrapper` in `StatementContext` is set based on the **per-variant** `wraps` field in `Statement`. When `#[wraps]` is at enum level, the toolkit propagates it to each variant's `Statement::wraps` field -- so `is_wrapper` should already be `true` for all variants.

However, examining the toolkit code in `crates/kirin-derive-toolkit/src/ir/statement/definition.rs:95-97`, the `from_variant` method takes a `wraps: bool` parameter from the parent enum. This means the toolkit already propagates enum-level `#[wraps]` to variants. The `Statement::wraps` field should be populated, and `StatementContext::is_wrapper` should be `true`.

**Key finding:** If the toolkit correctly propagates enum-level `#[wraps]`, then `derive(Interpretable)` should already work without code changes. The issue may be that the `wraps` field on `Statement` only gets set for **per-variant** wraps, not enum-level wraps, OR that the where-clause generation misses the wrapper types when wraps is at enum level.

### Manual impls for comparison

`Lexical<T>` in `crates/kirin-function/src/interpret_impl.rs:97-116`:
```rust
impl<'ir, I, T> Interpretable<'ir, I> for Lexical<T>
where I: Interpreter<'ir>, I::Value: Clone, T: CompileTimeValue,
{
    fn interpret<L>(...) {
        match self {
            Lexical::FunctionBody(op) => op.interpret::<L>(interp),
            Lexical::Lambda(op) => op.interpret::<L>(interp),
            Lexical::Call(op) => op.interpret::<L>(interp),
            Lexical::Return(op) => op.interpret::<L>(interp),
        }
    }
}
```

This is exactly the same delegation pattern that the derive generates for per-variant `#[wraps]`. The only difference is additional `I::Value: Clone` bounds on some impls (needed by `Call` and `Return`).

### SSACFGRegion interaction

`Lexical<T>` also has a manual `SSACFGRegion` impl that only forwards `entry_block` for `FunctionBody` and `Lambda` variants, returning `Err` for others. The `#[derive(SSACFGRegion)]` uses the `#[callable]` attribute to distinguish which variants forward. This already works correctly -- enum-level `#[wraps]` does not affect `SSACFGRegion` because `#[callable]` is always per-variant or enum-level independently.

### Where clause generation

The current derive generates:
```rust
where
    __InterpI: Interpreter<'__ir>,
    InnerTypeA: Interpretable<'__ir, __InterpI>,
    InnerTypeB: Interpretable<'__ir, __InterpI>,
    ...
```

For enum-level `#[wraps]`, the wrapper types come from each variant's single field. The `StatementContext::wrapper_type` should contain `FunctionBody<T>`, `Lambda<T>`, etc. If the toolkit correctly populates these, the where clause generation is already correct.

### Extra bounds problem

The manual `Lexical<T>` impl adds `I::Value: Clone`. The derive has no way to know this is needed. Options:
1. Add an `#[interpret(where(I::Value: Clone))]` attribute (was removed per AGENTS.md)
2. Always add common bounds like `I::Value: Clone` (too broad)
3. Accept that some enums need manual impls for extra bounds

Since `#[interpret(where(...))]` was removed, option 3 is the pragmatic choice. The derive covers the common case; enums needing extra bounds stay manual.

## Proposed Design

### Step 1: Verify toolkit propagation

Confirm that `Statement::wraps` is populated for each variant when `#[wraps]` is at enum level. If it already works, the only change needed is removing or relaxing the validation.

### Step 2: Adjust validation (if needed)

If the toolkit does NOT propagate enum-level `#[wraps]` to variant-level `Statement::wraps`, two approaches:

**Option A (toolkit change):** Modify `Input::from_derive_input` to propagate enum-level `#[wraps]` to each variant's `Statement::wraps`. This benefits all derive macros.

**Option B (derive-side):** In the interpretable derive, check for enum-level `#[wraps]` and treat all variants as wrappers. Access the top-level attribute from `ir.attrs` or `ir.extra_attrs`.

Recommendation: **Option A** -- the toolkit should handle this consistently.

### Step 3: Test with existing enums

Verify the derive produces correct code for `Lexical<T>` (4 variants, all wraps), `Lifted<T>` (4 variants), and `StructuredControlFlow<T>` (3 variants, one terminator).

### Code changes

In `crates/kirin-derive-interpreter/src/interpretable.rs`, the validation at line 52-67 already checks `is_wrapper` per statement. If the toolkit propagates correctly, no validation change is needed. If not, add a check:

```rust
.validate(|ctx| {
    // enum-level #[wraps] makes all variants wrappers
    if ctx.input.has_enum_wraps() {
        return Ok(());
    }
    // ... existing per-variant check
})
```

## Implementation Steps

1. **Write a test** in `kirin-derive-interpreter/src/interpretable.rs` that uses enum-level `#[wraps]`:
   ```rust
   #[test]
   fn test_interpretable_enum_level_wraps() {
       let input: syn::DeriveInput = syn::parse_quote! {
           #[kirin(type = SimpleType)]
           #[wraps]
           enum Composed {
               A(AOp),
               B(BOp),
           }
       };
       insta::assert_snapshot!(generate_interpretable_code(input));
   }
   ```

2. **Run the test** to see if it passes or fails. This determines whether the toolkit already propagates correctly.

3. **If test fails:** Trace through `Statement::from_variant(wraps: bool, ...)` to confirm the `wraps` parameter is used to set `Statement::wraps`. If the propagation path has a gap, fix it in the toolkit.

4. **If test passes:** The derive already works. Add the test, update documentation, and migrate `Lexical<T>` / `Lifted<T>` / `StructuredControlFlow<T>` if their bounds allow it (no extra `I::Value: Clone` etc.).

5. **Add corresponding `SSACFGRegion` test** with enum-level `#[wraps]` + `#[callable]` to confirm that derive also works.

6. **Documentation:** Update AGENTS.md to note that enum-level `#[wraps]` works with `#[derive(Interpretable)]`.

## Risk Assessment

**Low risk:**
- The toolkit likely already handles propagation (the `wraps: bool` parameter in `from_variant` suggests it does). This may be a documentation/test gap, not a code gap.
- Even if a toolkit change is needed, it only affects the `Statement::wraps` field -- no behavioral change for existing derives since they already handle this field.

**Medium risk:**
- Enums like `Lexical<T>` have extra bounds (`I::Value: Clone`) that the derive cannot infer. These enums cannot use the derive and must stay manual. This limits the practical benefit but is acceptable.

**Not a risk:**
- The `#[kirin(terminator)]` interaction with enum-level `#[wraps]` is a `Dialect` derive concern, not an `Interpretable` derive concern. The interpretable derive only generates delegation -- it does not inspect terminator status.

## Testing Strategy

- **Snapshot test**: enum-level `#[wraps]` produces correct delegation code (match arms with `inner.interpret::<__InterpL>(interpreter)`).
- **Snapshot test**: enum-level `#[wraps]` with generics (`Composed<T>`) propagates bounds correctly.
- **Compile test**: verify the generated code compiles against real types (may need a trybuild test or integration test in `kirin-function`).
- **Negative test**: enum-level `#[wraps]` with a variant that has additional fields beyond the wrapper field should error clearly.
