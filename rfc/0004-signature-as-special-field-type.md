# RFC 0004: Signature as a Special Field Type

- **Status**: Draft
- **Authors**: roger, claude
- **Created**: 2026-03-20

## Summary

Make `Signature<T>` a recognized special field type in the derive system, alongside `SSAValue`, `ResultValue`, `Block`, `Region`, `DiGraph`, and `UnGraph`. When a dialect struct declares a `Signature<T>` field, `#[derive(Dialect)]` generates `HasSignature` automatically. The parser populates the field from `{:signature}` or `{:return}` context projections. This eliminates the need for `extract_signature` on `ParseEmit` and the `manual_parse_emit` escape hatch.

## Motivation

### Current state

RFC 0003 introduced dialect-controlled function formats with context projections (`{:name}`, `{:return}`, `{:signature}`). For use case 3 (full dialect control), the function signature must be extracted from the parsed body. Currently this requires:

1. Adding `#[chumsky(manual_parse_emit)]` to opt out of derive-generated `ParseEmit`
2. Manually implementing `ParseEmit` with both `parse_and_emit` (delegating to derive helpers) and `extract_signature` (reading ports/yields from the IR)

```rust
// Current: dialect author must write this boilerplate
#[derive(Dialect, HasParser, PrettyPrint)]
#[chumsky(manual_parse_emit)]
struct CircuitFunction { body: DiGraph }

impl ParseEmit for CircuitLang {
    fn parse_and_emit(input: &str, ctx: &mut EmitContext<'_, Self>) -> Result<Statement, ChumskyError> {
        let ast = parse_ast::<Self>(input)?;
        HasParserEmitIR::emit_parsed(&ast, ctx).map_err(ChumskyError::Emit)
    }
    fn extract_signature(stmt: Statement, stage: &StageInfo<Self>) -> Option<Signature<QubitType>> {
        // 15+ lines of IR reading boilerplate
    }
}
```

### Problem

- The signature is a **property of the parsed statement**, not a runtime computation. It should be a field.
- Dialect authors must understand `HasParserEmitIR`, `parse_ast`, and IR arena reading to override `extract_signature`.
- The `manual_parse_emit` attribute is an escape hatch that bypasses the derive for an orthogonal concern.
- Every new dialect with function body support repeats the same IR-reading pattern.

### Desired state

```rust
#[derive(Dialect, HasParser, PrettyPrint)]
#[chumsky(format = "fn {:name}({body:ports}) captures ({body:captures}) -> {:return} {{ {body:body} }}")]
struct CircuitFunction {
    pub body: DiGraph,
    pub sig: Signature<QubitType>,  // ← derive handles everything
}
```

No manual `ParseEmit` impl. No `extract_signature`. No `manual_parse_emit`.

## Design

### Signature as a special field

`Signature<T>` joins the set of types recognized by the field categorization system in `kirin-derive-toolkit`:

| Type | Category | Recognized by |
|------|----------|---------------|
| `SSAValue` | Argument | `derive(Dialect)` + `derive(HasParser)` |
| `ResultValue` | Result | `derive(Dialect)` + `derive(HasParser)` |
| `Block` | Block | `derive(Dialect)` + `derive(HasParser)` |
| `Region` | Region | `derive(Dialect)` + `derive(HasParser)` |
| `DiGraph` | DiGraph | `derive(Dialect)` + `derive(HasParser)` |
| `UnGraph` | UnGraph | `derive(Dialect)` + `derive(HasParser)` |
| **`Signature<T>`** | **Signature** | **`derive(Dialect)` + `derive(HasParser)`** |

**File:** `kirin-derive-toolkit/src/ir/fields/data.rs`

Add `FieldCategory::Signature` and `FieldData::Signature`.

**File:** `kirin-derive-toolkit/src/ir/statement/definition.rs`

Add recognition in `parse_field`: detect `Signature<T>` type, extract `T`, produce `FieldData::Signature`.

### HasSignature changes

**File:** `kirin-ir/src/signature/has_signature.rs`

Change the return type to `Option<Signature<L::Type>>`:

```rust
pub trait HasSignature<L: Dialect> {
    fn signature(&self, stage: &StageInfo<L>) -> Option<Signature<L::Type>>;
}
```

- Structs with a `Signature<T>` field: `derive(Dialect)` generates `Some(self.sig.clone())`
- Structs without: `derive(Dialect)` generates `None`
- `#[wraps]` enums: delegates to inner type
- Non-wraps enums: per-variant dispatch, `None` for non-function variants

### Parser codegen

**File:** `kirin-derive-chumsky/src/codegen/parser/chain.rs`

When the format string contains `{:return}`, the parsed return types are **stored in the Signature field** instead of discarded:

- `{:return}` → parse `Type, Type` → combined with port types from `{body:ports}` and `{body:captures}` → construct `Signature<T>` → assign to field
- `{:signature}` → parse `fn @name(Type, Type) -> Type` → construct `Signature<T>` → assign to field

The Signature field is populated in the AST constructor, similar to how `ResultValue` fields get their `result_index`.

### Printer codegen

**File:** `kirin-derive-chumsky/src/codegen/pretty_print/statement.rs`

The Signature field provides data for context projections:

- `{:return}` reads `self.sig.ret()` to print return types
- `{:signature}` reads `self.sig` to print `fn @name(params) -> ret`
- `{:name}` continues to read from `Document::function_name()` (set by framework)

This eliminates the need for `Document::return_type_text` and `Document::signature_text` — the data comes from the field, not from framework context.

### Function text parser

**File:** `kirin-chumsky/src/function_text/parse_text.rs`

Replace `L::extract_signature()` with `HasSignature`:

```rust
let signature = if let Some(sig) = framework_signature {
    sig.clone()
} else {
    let def = body_statement.expect_info(stage).definition();
    def.signature(stage).ok_or_else(|| /* error: no signature */)?
};
```

The `L: HasSignature<L>` bound is added to `apply_specialize_declaration` (and propagated through dispatch). Since `derive(Dialect)` generates it for all types, this is automatically satisfied.

### What gets removed

- `extract_signature` method from `ParseEmit`
- `manual_parse_emit` attribute from `ChumskyGlobalAttrs`
- `Document::return_type_text` and `Document::signature_text` context state
- `set_return_type_text` / `set_signature_text` / `return_type_text` / `signature_text` on `Document`
- `print_return_types()` and `print_function_signature()` methods (replaced by field-based printing)

## Examples

### Use case 2: Framework signature + body projections

```rust
#[derive(Dialect, HasParser, PrettyPrint)]
#[kirin(type = SimpleType)]
#[chumsky(format = "{:signature} ({body:ports}) captures ({body:captures}) {{ {body:body} }}")]
struct ProjectedFunc {
    body: DiGraph,
    sig: Signature<SimpleType>,  // populated from {:signature}
}
```

`{:signature}` parses `fn @name(Type, Type) -> Type` and stores the result in `sig`.

### Use case 3: Full dialect control

```rust
#[derive(Dialect, HasParser, PrettyPrint)]
#[kirin(type = QubitType)]
#[chumsky(format = "fn {:name}({body:ports}) captures ({body:captures}) -> {:return} {{ {body:body} }}")]
struct CircuitFunction {
    body: DiGraph,
    sig: Signature<QubitType>,  // populated from ports + captures + {:return}
}
```

`sig.params()` = types from `{body:ports}` ++ `{body:captures}`. `sig.ret()` = type from `{:return}`.
`HasSignature` generated automatically: `Some(self.sig.clone())`.

### Non-function statement (no Signature field)

```rust
#[derive(Dialect, HasParser, PrettyPrint)]
#[kirin(type = QubitType)]
#[chumsky(format = "$h {qubit}")]
struct H { qubit: SSAValue }
```

`HasSignature` generated: `None`. Cannot be used as a function body.

## Alternatives

### Alternative A: Keep `extract_signature` + `manual_parse_emit`

Keep the current design where dialect authors manually implement `ParseEmit` to override `extract_signature`.

**Rejected:** Requires boilerplate, exposes internal implementation details (`HasParserEmitIR`, `parse_ast`), and every new dialect repeats the same pattern.

### Alternative B: Derive-generated `extract_signature` from IR

Have the derive generate `extract_signature` that reads ports/yields directly from the IR, without a Signature field.

**Rejected:** The signature is a parsed value, not a derived computation. Storing it as a field is more explicit, follows the SSAValue/ResultValue pattern, and avoids re-reading the IR at extraction time.

## Crate impact matrix

| Crate | Impact | Changes |
|-------|--------|---------|
| `kirin-ir` | **Primary** | `HasSignature` returns `Option`, re-export `Signature` in prelude |
| `kirin-derive-toolkit` | **Primary** | `FieldCategory::Signature`, `FieldData::Signature`, recognition logic |
| `kirin-derive-ir` | **Primary** | Generate `HasSignature` impl from `derive(Dialect)` |
| `kirin-derive-chumsky` | **Primary** | Parser populates Signature field, printer reads from it, remove `manual_parse_emit` and `extract_signature` |
| `kirin-chumsky` | **Secondary** | Function text parser uses `HasSignature` instead of `extract_signature`, remove `extract_signature` from `ParseEmit` |
| `kirin-prettyless` | **Secondary** | Remove `return_type_text` / `signature_text` context from `Document` |
| `example/toy-qc` | **Migration** | Add `sig: Signature<QubitType>` field, remove manual `HasSignature` impls |
| All test languages | **Migration** | Add `sig: Signature<T>` field to function body variants |

## Migration path

1. Add `FieldCategory::Signature` and recognition to derive-toolkit
2. Generate `HasSignature` from `derive(Dialect)` — change return to `Option`
3. Update parser codegen to populate Signature field from context projections
4. Update printer codegen to read from Signature field
5. Remove `extract_signature` from `ParseEmit`, remove `manual_parse_emit`
6. Remove Document context state (`return_type_text`, `signature_text`)
7. Migrate toy-qc: add `sig` field, remove manual `HasSignature` impls
8. Migrate test languages: add `sig` field to function body variants

## Open questions

1. **Signature field naming convention:** Should it be `sig`, `signature`, or configurable via attribute? Convention-based (`sig` or `signature`) is simplest.

2. **Signature population for `{:signature}` vs `{:return}`:** For `{:signature}`, the full `fn @name(Type, Type) -> Type` is parsed — the Signature gets params + ret. For `{:return}`, only the return type is parsed — params come from ports + captures. Should the derive handle both cases, or should `{:return}` require explicit port-to-param mapping?

3. **HasSignature on language enums:** Should `derive(Dialect)` on a language enum generate `HasSignature<L> for L` that dispatches per-variant? Or should the `derive(HasParser)` / dispatch machinery handle this?

4. **Backward compatibility of HasSignature return type change:** Changing `Signature<L::Type>` → `Option<Signature<L::Type>>` breaks all existing impls. Should we add a migration period with a separate trait, or is a clean break acceptable?
