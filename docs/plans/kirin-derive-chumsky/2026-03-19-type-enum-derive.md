# Type Enum Derive via `chumsky(format = ...)`

## Problem

Type enums like `ArithType` require four manual trait implementations: `Display`, `HasParser`, `PrettyPrint`, and `DirectlyParsable`. Each impl is pure boilerplate -- mapping variant names to string literals and back. For `ArithType` alone this is ~60 lines of repetitive code across the four impls. The existing `#[derive(HasParser, PrettyPrint)]` only targets dialect statement enums (which emit IR), not simple value/type enums that parse directly into themselves.

A format-string-based approach (rather than simple keyword mapping) is needed because type enums can be generic (e.g., a user-defined `MyType<T>`) and the format DSL already exists for statement structs.

### Current manual code (ArithType, 60+ lines)

- `Display`: match arm per variant returning a string literal
- `HasParser<'t>`: `select!` macro mapping `Token::Identifier("i32") => ArithType::I32` per variant
- `PrettyPrint`: delegates to `doc.text(self.to_string())`
- `DirectlyParsable`: empty marker impl

### Desired ergonomic

```rust
#[derive(HasParser, PrettyPrint)]
#[chumsky(format = "type")]  // signals "this is a type enum, not a statement enum"
pub enum ArithType {
    #[chumsky(format = "i8")]
    I8,
    #[chumsky(format = "i16")]
    I16,
    // ...
}
```

This should generate all four impls automatically.

## Research Findings

### Existing format string infrastructure

The `Format` parser in `crates/kirin-derive-chumsky/src/format.rs` handles `FormatElement::Token`, `FormatElement::Field`, and `FormatElement::Keyword`. For type enums, variant formats would only use the `Token` element (literal identifiers like `"i32"`). The existing parser already handles bare identifier tokens, so no DSL extension is needed.

### Code generation paths

Current `GenerateHasDialectParser::generate()` produces:
1. `HasDialectParser<'t>` impl (namespaced parser with `Output` type)
2. `HasParser<'t>` impl (delegates to dialect parser)
3. `ParseEmit<L>` impl (for text APIs)
4. `HasDialectEmitIR` impl (dialect-level emit)

For type enums, we need a different code generation path:
1. `HasParser<'t>` impl (direct parser, not dialect-namespaced)
2. `Display` impl (from format strings)
3. `PrettyPrint` impl (blanket via `doc.text(self.to_string())`)
4. `DirectlyParsable` marker impl
5. `EmitIR` -- gets blanket impl from `DirectlyParsable`

### Detection mechanism

Two options for detecting type-enum mode:
- **Option A**: Top-level `#[chumsky(format = "type")]` -- overloads the format attribute
- **Option B**: Separate attribute like `#[chumsky(kind = "type")]` -- clearer intent

Recommendation: **Option B** with `#[chumsky(mode = "type")]` or simply detect automatically: if all variants are unit variants (no fields) and each has a `#[chumsky(format = "...")]` containing only literal tokens, treat it as a type enum. This avoids any new top-level attribute.

### Generic type enums

Generic type enums like `MyType<T>` would use format strings that don't reference `T` (since `T` is a phantom). The derive would propagate generic parameters through all generated impls. The existing `ir_input.generics` handling already does this for statement derives.

### Traits to generate

| Trait | What it generates |
|-------|-------------------|
| `Display` | `match self { Self::I8 => write!(f, "i8"), ... }` |
| `HasParser<'t>` | `select! { Token::Identifier("i8") => Self::I8, ... }.boxed()` |
| `DirectlyParsable` | `impl DirectlyParsable for T {}` |
| `PrettyPrint` | `doc.text(self.to_string())` |

### What `DirectlyParsable` requires

`DirectlyParsable: Clone` (marker trait in `kirin-chumsky/src/traits/emit_ir.rs:131`). It provides a blanket `EmitIR<L>` impl that returns `Ok(self.clone())`. The type enum must already derive `Clone`.

## Proposed Design

### Attribute syntax

```rust
#[derive(HasParser, PrettyPrint)]
pub enum ArithType {
    #[chumsky(format = "i8")]
    I8,
    #[chumsky(format = "i16")]
    I16,
    // ...
}
```

No top-level `#[chumsky(...)]` needed. Detection is automatic: if the derive input has no `#[kirin(type = ...)]`, all variants are unit (fieldless), and each variant has `#[chumsky(format = "...")]`, enter type-enum codegen mode.

The existing `parse_derive_input` function in `crates/kirin-derive-chumsky/src/input.rs` already handles missing `#[kirin(type = ...)]` by patching in a placeholder -- this same path would route to type-enum codegen.

### Code generation

Add a new module `crates/kirin-derive-chumsky/src/codegen/type_enum.rs` with:

```rust
pub struct GenerateTypeEnum;

impl GenerateTypeEnum {
    pub fn generate(ir_input: &Input<ChumskyLayout>) -> TokenStream {
        let display_impl = Self::generate_display(ir_input);
        let has_parser_impl = Self::generate_has_parser(ir_input);
        let directly_parsable_impl = Self::generate_directly_parsable(ir_input);
        quote! { #display_impl #has_parser_impl #directly_parsable_impl }
    }
}
```

The `PrettyPrint` derive would similarly detect type-enum mode and generate `doc.text(self.to_string())`.

### Routing logic

In `crates/kirin-derive-chumsky/src/lib.rs`, the `HasParser` derive entry point would check:

```rust
fn is_type_enum(input: &Input<ChumskyLayout>) -> bool {
    match &input.data {
        Data::Enum(data) => data.variants.iter().all(|v| {
            v.wraps.is_none()
                && v.collect_fields().is_empty()
                && v.extra_attrs.format.is_some()
        }),
        _ => false,
    }
}
```

### Multi-token formats

A variant format like `#[chumsky(format = "my_type")]` maps to `Token::Identifier("my_type")`. For multi-token formats (unlikely for types but possible), the format parser already handles sequences of tokens -- the codegen would chain `.then()` calls.

### Label

The generated parser would include `.labelled("arith type")` using a kebab-case transformation of the enum name.

## Implementation Steps

1. **Add `is_type_enum` detection** in `crates/kirin-derive-chumsky/src/lib.rs` or a shared utility. Check: no fields, no `#[wraps]`, all variants have format strings, no `#[kirin(type = ...)]` (or placeholder type).

2. **Create `codegen/type_enum.rs`** with three generators:
   - `generate_display`: Match arms mapping variants to format string literals
   - `generate_has_parser`: `select!` macro with `Token::Identifier(lit) => Variant` arms
   - `generate_directly_parsable`: Empty marker impl

3. **Route `HasParser` derive** through `GenerateTypeEnum` when `is_type_enum()` is true. Keep existing `GenerateHasDialectParser` for statement enums.

4. **Route `PrettyPrint` derive** through a simple `doc.text(self.to_string())` generator when `is_type_enum()` is true.

5. **Add tests**: snapshot tests for generated code, roundtrip test using derived `ArithType`.

6. **Migrate `ArithType`** to use the derive, removing ~60 lines of manual code.

7. **Migrate other type enums**: `ArithValue`, test types (`Value`, `UnitType`, `SimpleType`) where applicable.

## Risk Assessment

**Low risk:**
- The type-enum path is entirely separate from the statement-enum path -- no regression risk to existing derives.
- Detection heuristic (unit variants + format + no `#[kirin(type)]`) is unambiguous -- no existing enum matches this pattern accidentally.

**Medium risk:**
- `Display` generation from a parser derive macro is unusual. Users might expect `Display` to come from a separate derive. Mitigation: document clearly that `#[derive(HasParser)]` on type enums generates `Display` as well, or require users to write their own `Display` and only generate `HasParser + DirectlyParsable`.

**Decision point:** Should the derive generate `Display`, or require users to write it separately? Generating it is more ergonomic but couples `Display` to the parser crate. A compromise: generate `Display` only when a type-level `#[chumsky(display)]` flag is present.

## Testing Strategy

- **Snapshot tests** in `crates/kirin-derive-chumsky/src/codegen/type_enum.rs`: verify generated `Display`, `HasParser`, `DirectlyParsable`, and `PrettyPrint` code for a representative type enum.
- **Roundtrip test** in `tests/roundtrip/`: parse `ArithType` from text, print it back, compare.
- **Compile-fail test**: enum with fields + type-enum format should produce a clear error.
- **Generic type enum test**: ensure `MyType<T>` propagates generics correctly.
