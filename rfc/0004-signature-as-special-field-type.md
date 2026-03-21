# RFC 0004: Signature as a Special Field Type

- **Status**: Draft
- **Authors**: roger, claude
- **Created**: 2026-03-20

## Summary

Make `Signature<T>` a recognized special field type in the derive system, alongside `SSAValue`, `ResultValue`, `Block`, `Region`, `DiGraph`, and `UnGraph`. Recognition is **by type**, not by field name. When a dialect struct declares a `Signature<T>` field, `#[derive(Dialect)]` generates `HasSignature` automatically. `Signature<T>` implements `HasParser` so it can appear as `{field}` in format strings — replacing the `{:signature}` context projection entirely. It also supports field projections `{sig:inputs}` and `{sig:return}` for dialects that split the signature across the format. This eliminates `extract_signature` on `ParseEmit`, the `manual_parse_emit` escape hatch, and the `{:signature}` / `{:return}` context projections. `{:name}` remains as the sole framework context projection — the function name is an identity owned by the abstract function, not a parsed property of the statement.

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
- `{:signature}` is a magic context projection that conflates the function name with the type signature. The function name is a framework concept (from `stage`/`specialize` headers), not a statement property.
- Dialect authors must understand `HasParserEmitIR`, `parse_ast`, and IR arena reading to override `extract_signature`.
- The `manual_parse_emit` attribute is an escape hatch that bypasses the derive for an orthogonal concern.
- Every new dialect with function body support repeats the same IR-reading pattern.

### Desired state

```rust
// Use case 1: Region body — whole signature parsed as a unit
#[derive(Dialect, HasParser, PrettyPrint)]
#[chumsky(format = "fn {:name}{sig} {body}")]
struct FunctionBody<T: CompileTimeValue> {
    pub body: Region,
    pub sig: Signature<T>,  // {sig} parses (T, T) -> T
}

// Use case 3: DiGraph body — signature and ports are separate concerns
#[derive(Dialect, HasParser, PrettyPrint)]
#[chumsky(format = "gate {:name}({sig:inputs}) -> {sig:return} ({body:ports}) captures ({body:captures}) {{ {body:body} }}")]
struct CircuitFunction {
    pub body: DiGraph,
    pub sig: Signature<QubitType>,  // {sig:inputs} + {sig:return} for type signature
    // {body:ports} + {body:captures} for IR structure — types appear in both
}
```

No manual `ParseEmit` impl. No `extract_signature`. No `manual_parse_emit`.

## Design

### Signature as a special field

`Signature<T>` joins the set of types recognized by the field categorization system in `kirin-derive-toolkit`. Recognition is **by type path** — the field name is arbitrary (`sig`, `signature`, `my_sig`, etc.), following the same convention as `body: Region` vs `my_region: Region`.

| Type | Category | Recognized by |
|------|----------|---------------|
| `SSAValue` | Argument | `derive(Dialect)` + `derive(HasParser)` |
| `ResultValue` | Result | `derive(Dialect)` + `derive(HasParser)` |
| `Block` | Block | `derive(Dialect)` + `derive(HasParser)` |
| `Region` | Region | `derive(Dialect)` + `derive(HasParser)` |
| `DiGraph` | DiGraph | `derive(Dialect)` + `derive(HasParser)` |
| `UnGraph` | UnGraph | `derive(Dialect)` + `derive(HasParser)` |
| **`Signature<T>`** | **Signature** | **`derive(Dialect)` + `derive(HasParser)`** |

**Constraint: at most one `Signature<T>` field per struct or enum variant.** The derive errors if it sees two. This mirrors the Region/DiGraph/UnGraph constraint.

**Scope: only top-level function body types.** `Signature<T>` is for types that serve as the body of a `specialize` declaration in function text (e.g., `FunctionBody`, `CircuitFunction`). Nested statements like `Lambda` — which contain a Region but are used within other statements, not at the top level — should NOT have a Signature field.

**File:** `kirin-derive-toolkit/src/ir/fields/data.rs`

Add `FieldCategory::Signature` and `FieldData::Signature { inner_type: syn::Type }`.

**File:** `kirin-derive-toolkit/src/ir/statement/definition.rs`

Add recognition in `parse_field`: detect `Signature<T>` type path, extract `T`, produce `FieldData::Signature`.

### HasParser for Signature

`Signature<T>` implements `HasParser` so it can be used as a regular field reference `{sig}` in format strings. The parsed syntax is:

```
(Type, Type, ...) -> Type
```

Params in parentheses, comma-separated, arrow, single return type.

```rust
impl<'t, T> HasParser<'t> for Signature<T>
where
    T: HasParser<'t, Output = T>,
{
    type Output = Signature<T>;

    fn parser() -> impl Parser<'t, I, Self::Output, ParserError<'t>> {
        T::parser()
            .separated_by(just(Token::Comma))
            .collect::<Vec<_>>()
            .delimited_by(just(Token::LParen), just(Token::RParen))
            .then_ignore(just(Token::Arrow))
            .then(T::parser())
            .map(|(params, ret)| Signature::new(params, ret, ()))
    }
}
```

**File:** `kirin-ir/src/signature/definition.rs` (or a new `kirin-chumsky/src/impls/signature.rs`)

Note: This is the same logic currently in `fn_params_and_return` in `kirin-chumsky/src/function_text/syntax.rs`, but moved to a `HasParser` impl on the type itself.

`PrettyPrint` (or `Display`) for `Signature<T>` outputs the same format: `(T, T) -> T`.

### Signature field projections

Like graph fields (`{body:ports}`, `{body:body}`), the Signature field supports projections that decompose it into parts:

| Projection | Parses | Prints | Type |
|------------|--------|--------|------|
| `{sig}` | `(T, T) -> T` (whole) | `(T, T) -> T` | `Signature<T>` |
| `{sig:inputs}` | `T, T` (comma-separated type list) | `T, T` | `Vec<T>` → `sig.params()` |
| `{sig:return}` | `T` (single type) | `T` | `T` → `sig.ret()` |

**Constraint:** Either use `{sig}` as a whole, or use `{sig:inputs}` + `{sig:return}` together. The derive errors if only one projection appears without the other. This ensures the Signature field is always fully determined by the format string.

Valid forms:
- `{sig}` alone — whole signature
- `{sig:inputs}` + `{sig:return}` — split signature, both parsed from text

Invalid:
- `{sig:inputs}` without `{sig:return}` — return type unknown
- `{sig:return}` without `{sig:inputs}` — input types unknown
- `{sig}` combined with `{sig:inputs}` or `{sig:return}` — ambiguous

**Parser codegen for projections:**

- `{sig:inputs}` generates `T::parser().separated_by(just(Token::Comma)).collect::<Vec<_>>()`
- `{sig:return}` generates `T::parser()`
- After both are parsed, the codegen constructs `Signature::new(inputs, ret, ())`

**Printer codegen for projections:**

- `{sig:inputs}` generates a comma-separated rendering of `self.sig.params()`
- `{sig:return}` generates `format!("{}", self.sig.ret())`

### Eliminating `{:signature}` and `{:return}`

The `{:signature}` and `{:return}` context projections are replaced by field references and field projections:

| Before | After |
|--------|-------|
| `"{:signature} {body}"` | `"fn {:name}{sig} {body}"` |
| `"fn {:name}(...) -> {:return} ..."` | `"fn {:name}({sig:inputs}) -> {sig:return} ..."` |
| `{:signature}` conflated name + signature from framework context | `{sig}` parses `(T, T) -> T` via `HasParser`; `{:name}` handles name separately |
| `{:return}` populated by framework | `{sig:return}` parses `T` from text, stored in Signature field |
| `Document::signature_text` / `Document::return_type_text` | `sig` field printed via `PrettyPrint` / projections |

**`{:name}` stays as a framework context projection.** The function name is an identity owned by the abstract function (see resolved question 4). During printing, `{:name}` reads from `Document::function_name()`. During parsing, `{:name}` consumes the `@symbol` token and stores the name in `EmitContext::function_name` — the function text parser reads it back after `parse_and_emit` for Function/StagedFunction lookup.

Dialect developers control the keyword and name placement via the format string. `fn {:name}(...)`, `gate {:name}(...)`, or any other pattern — the framework doesn't impose a keyword.

### HasSignature changes

**File:** `kirin-ir/src/signature/has_signature.rs`

Change the return type to `Option<Signature<L::Type>>`:

```rust
pub trait HasSignature<L: Dialect> {
    fn signature(&self, stage: &StageInfo<L>) -> Option<Signature<L::Type>>;
}
```

`derive(Dialect)` generates the impl:

- **Structs with a `Signature<T>` field:** `Some(self.sig.clone())`
- **Structs without:** `None`
- **`#[wraps]` enums:** Delegate to inner type (which may return `Some` or `None`)
- **Non-wraps enums:** Per-variant match — `Some(sig.clone())` for variants with a Signature field, `None` otherwise

```rust
// Generated for a non-wraps enum:
impl HasSignature<ArithFunctionLanguage> for ArithFunctionLanguage {
    fn signature(&self, _stage: &StageInfo<Self>) -> Option<Signature<ArithType>> {
        match self {
            Self::Function { sig, .. } => Some(sig.clone()),
            _ => None,
        }
    }
}
```

This is generated by `derive(Dialect)` in `kirin-derive-ir`, not by `derive(HasParser)` in `kirin-derive-chumsky`. `HasSignature` is an IR-level trait. The field categorization comes from `kirin-derive-toolkit` which is shared.

**Backward compatibility:** Clean break. The only existing manual impls are `CircuitFunction` and `ZXFunction` in `toy-qc`. Both are **deleted** after migration (replaced by a `sig` field + derive-generated impl). Wrapping the return in `Some()` is trivial for any external code.

### Whole vs projected population

The Signature field is always populated from the format string — either as a whole or from projections:

**Whole — `{sig}` in format string:**

```rust
#[chumsky(format = "fn {:name}{sig} {body}")]
struct FunctionBody<T> { body: Region, sig: Signature<T> }
```

The parser parses `{sig}` from text `(T, T) -> T` via `Signature<T>::HasParser`. This is the common case for Region-based functions.

**Projected — `{sig:inputs}` + `{sig:return}` in format string:**

```rust
#[chumsky(format = "gate {:name}({sig:inputs}) -> {sig:return} ({body:ports}) captures ({body:captures}) {{ {body:body} }}")]
struct CircuitFunction { body: DiGraph, sig: Signature<QubitType> }
```

The parser parses `{sig:inputs}` as a comma-separated type list and `{sig:return}` as a single type. The codegen constructs `Signature::new(inputs, ret, ())` from the two parsed values. This is for graph-based functions where the signature projections and body projections are separate concerns — `{sig:inputs}` / `{sig:return}` for the type interface, `{body:ports}` / `{body:captures}` for the IR structure. Types appear in both, mirroring MLIR's separation of function type from block argument declarations.

**Validation:** The derive checks which Signature references appear in the format string and validates one of the two valid forms. A Signature field that has no corresponding `{sig}`, `{sig:inputs}`, or `{sig:return}` in the format string is a compile error.

### Parser codegen

**File:** `kirin-derive-chumsky/src/codegen/parser/chain.rs`

**Whole (`{sig}`):** Treated as a regular field reference. The parser chain includes `Signature<T>::parser()` at the appropriate position. The Signature is part of the AST, same as any other parsed field.

**Projected (`{sig:inputs}` + `{sig:return}`):** Each projection is an independent parser in the chain:
- `{sig:inputs}` → `T::parser().separated_by(just(Token::Comma)).collect::<Vec<_>>()`
- `{sig:return}` → `T::parser()`
- After both are parsed, the AST constructor combines them: `Signature::new(inputs, ret, ())`

### Printer codegen

**File:** `kirin-derive-chumsky/src/codegen/pretty_print/statement.rs`

**Whole (`{sig}`):** Calls `self.sig.pretty_print(doc)` which outputs `(T, T) -> T`. The framework wraps it with `fn @name` when printing the function header.

**Projected (`{sig:inputs}` + `{sig:return}`):** Each projection reads from the field:
- `{sig:inputs}` → comma-separated rendering of `self.sig.params()`
- `{sig:return}` → `format!("{}", self.sig.ret())`

`Document::signature_text` and `Document::return_type_text` are removed — the data comes from the Signature field.

### Function text parser simplification

**File:** `kirin-chumsky/src/function_text/parse_text.rs`

The function text parser currently has two `specialize` paths:

- `specialize_with_sig`: framework pre-parses `fn @name(T, T) -> T`, strips signature, passes only `{ body }` to `parse_and_emit`
- `specialize_dialect`: framework can't parse the signature, passes full text from `fn` onward to `parse_and_emit`

With RFC 0004, the statement parser always handles keyword + name + signature + body via its format string. The function text parser only strips `specialize @stage` and delegates everything else.

**Unified specialize path:**

1. Parse `specialize @stage` — extract stage symbol
2. Pass the full remaining text (from the keyword onward) to `L::parse_and_emit()` — the statement parser handles keyword, `{:name}`, signature, and body
3. Read the function name from `EmitContext` — `{:name}` stored it during parsing
4. Look up / create Function + StagedFunction using the name
5. Read signature from the parsed body via `HasSignature`
6. Create Specialization

```rust
// Unified specialize path — parse first, then extract name and signature
let body_statement = L::parse_and_emit(rest_text, &mut emit_ctx)?;
let function_name = emit_ctx.function_name()
    .ok_or_else(|| /* error: format string must include {:name} for function body types */)?;
let signature = body_statement
    .expect_info(stage)
    .definition()
    .signature(stage)
    .ok_or_else(|| /* error: no Signature field on body type */)?;
```

This works because `parse_and_emit` doesn't need the Function/StagedFunction — the builder creates IR nodes within the stage context, and the function assignment happens in step 6 when creating the Specialization. The name is only needed afterward for staged_lookup.

The `L: HasSignature<L>` bound is added to `apply_specialize_declaration`. Since `derive(Dialect)` generates it for all types, this is automatically satisfied.

**Function name extraction via `EmitContext`:** During parsing, the `{:name}` element consumes the `@symbol` token and stores the name in `EmitContext::function_name`. This is format-string-aware by construction — the derive generates the `{:name}` parse element at the correct position in the parser chain, regardless of what keyword or tokens precede it. No scanning heuristic needed.

**What stays unchanged:**
- The **two-pass architecture** (pass 1: stage headers, pass 2: specialize bodies). This exists for forward references — `specialize` before `stage` is valid — and is orthogonal to signature handling.
- **`stage` declarations** still use framework signature parsing (`fn_signature_parser` in `syntax.rs`). A `stage` declaration creates a `StagedFunction` with a signature before any body is parsed. The `stage` syntax is fixed: `stage @stage fn @name(T, T) -> T;`.

**What gets simplified:**
- `specialize_with_sig` / `specialize_dialect` split in `declaration_parser` → unified into a single `specialize` path
- `framework_signature: Option<&Signature<L::Type>>` parameter → removed from `apply_specialize_declaration`
- Function name extraction → moved from framework scanning into `EmitContext` (stored by `{:name}` during the statement parse)
- `fn_params_and_return()` helper in `syntax.rs` → removed (absorbed into `Signature<T>::HasParser`)
- Body span scanning for specialize → simplified to "everything after `specialize @stage`"

**File:** `kirin-chumsky/src/function_text/syntax.rs`

The `declaration_parser` simplifies from two `specialize` alternatives to one:

```rust
// Before: two alternatives with framework signature pre-parsing
let specialize_decl = specialize_with_sig.or(specialize_dialect);

// After: one path — framework only strips `specialize @stage`, rest goes to statement parser
let specialize_decl = identifier("specialize")
    .ignore_then(symbol())                    // stage name
    .then(rest_span())                        // everything from keyword through closing }
    .map_with(|(stage, rest), extra| Declaration::Specialize {
        stage,
        rest_span: rest,
        span: extra.span(),
    });
```

`Declaration::Specialize` drops both `function: SymbolName` and `signature: Option<Signature<T>>` — the function name comes from `EmitContext::function_name()` (stored by `{:name}` during parsing), and the signature comes from the parsed body via `HasSignature`.

### What gets removed

**Format string system:**
- `{:signature}` context projection (replaced by `{sig}` field reference)
- `{:return}` context projection (replaced by `{sig:return}` field projection)

**`ParseEmit` trait:**
- `extract_signature` method
- `manual_parse_emit` attribute from `ChumskyGlobalAttrs`

**`Document` (pretty printer context):**
- `signature_text` / `return_type_text` fields and their getters/setters
- `print_function_signature()` method (replaced by `{sig}` / Signature's `PrettyPrint`)
- `print_return_types()` method (replaced by `{sig:return}` projection)

**Function text parser:**
- `specialize_with_sig` / `specialize_dialect` split → unified into one path
- `framework_signature` parameter on `apply_specialize_declaration`
- `fn_params_and_return()` helper in `syntax.rs` (absorbed into `Signature<T>::HasParser`)
- `Declaration::Specialize::signature` field (signature comes from body's `HasSignature`)

**What stays:**
- `{:name}` — framework context projection for the function name (see resolved question 4)
- `Document::function_name()` — still needed for `{:name}` printing

## Examples

### Use case 1: Region body with whole signature

```rust
#[derive(Dialect, HasParser, PrettyPrint)]
#[kirin(type = T)]
#[chumsky(format = "fn {:name}{sig} {body}")]
struct FunctionBody<T: CompileTimeValue> {
    body: Region,
    sig: Signature<T>,  // whole: parsed from text via HasParser
}
```

Format string `fn {:name}{sig} {body}`:
- `fn ` — literal keyword
- `{:name}` — parses/prints `@foo` (framework context)
- `{sig}` — parses `(T, T) -> T` via `Signature<T>::HasParser`
- `{body}` — parses the Region

Function text: `specialize @stage fn @foo(T, T) -> T { body }`
- Framework strips `specialize @stage`
- Statement parser sees `fn @foo(T, T) -> T { body }`
- Statement printer outputs `fn @foo(T, T) -> T { body }`

### Use case 2: Whole signature + body projections

```rust
#[derive(Dialect, HasParser, PrettyPrint)]
#[kirin(type = SimpleType)]
#[chumsky(format = "fn {:name}{sig} ({body:ports}) captures ({body:captures}) {{ {body:body} }}")]
struct ProjectedFunc {
    body: DiGraph,
    sig: Signature<SimpleType>,  // whole: parsed from text
}
```

`{sig}` parses `(SimpleType, SimpleType) -> SimpleType`. Body projections control graph rendering separately.

### Use case 3: DiGraph body with projected signature + body projections

```rust
#[derive(Dialect, HasParser, PrettyPrint)]
#[kirin(type = QubitType)]
#[chumsky(format = "gate {:name}({sig:inputs}) -> {sig:return} ({body:ports}) captures ({body:captures}) {{ {body:body} }}")]
struct CircuitFunction {
    body: DiGraph,
    sig: Signature<QubitType>,  // projected: inputs + return parsed separately
}
```

Text: `gate @foo(Qubit, Qubit) -> Qubit (%p0: Qubit, %p1: Qubit) captures () { ... yield %r; }`

- `gate` — dialect-specific keyword (not `fn`)
- `{:name}` — parses `@foo`
- `{sig:inputs}` — parses `Qubit, Qubit` (type signature)
- `{sig:return}` — parses `Qubit` (return type)
- `{body:ports}` — parses `%p0: Qubit, %p1: Qubit` (IR port bindings)
- `{body:captures}` — parses empty
- `{body:body}` — parses the graph body

Types appear in both `{sig:inputs}` and `{body:ports}` — they serve different purposes: the type signature is the abstract interface, the ports are the concrete IR bindings. This mirrors the MLIR convention of separating function type from block argument declarations.

`HasSignature` generated: `Some(self.sig.clone())`.

### Non-function statement (no Signature field)

```rust
#[derive(Dialect, HasParser, PrettyPrint)]
#[kirin(type = QubitType)]
#[chumsky(format = "$h {qubit}")]
struct H { qubit: SSAValue }
```

`HasSignature` generated: `None`. Cannot be used as a function body.

### Enum with function variant

```rust
#[derive(Dialect, HasParser, PrettyPrint)]
#[kirin(type = ArithType)]
enum ArithFunctionLanguage {
    #[chumsky(format = "fn {:name}{sig} {body}")]
    Function { body: Region, sig: Signature<ArithType> },
    #[wraps]
    Arith(Arith<ArithType>),
    #[wraps]
    ControlFlow(ControlFlow<ArithType>),
    #[wraps]
    Return(Return<ArithType>),
}
```

`HasSignature` generated with per-variant dispatch:
- `Function { sig, .. }` → `Some(sig.clone())`
- All other variants → `None` (delegated through `#[wraps]`)

## Alternatives

### Alternative A: Keep `extract_signature` + `manual_parse_emit`

Keep the current design where dialect authors manually implement `ParseEmit` to override `extract_signature`.

**Rejected:** Requires boilerplate, exposes internal implementation details (`HasParserEmitIR`, `parse_ast`), and every new dialect repeats the same pattern.

### Alternative B: Derive-generated `extract_signature` from IR

Have the derive generate `extract_signature` that reads ports/yields directly from the IR, without a Signature field.

**Rejected:** The signature is a parsed value, not a derived computation. Storing it as a field is more explicit, follows the SSAValue/ResultValue pattern, and avoids re-reading the IR at extraction time.

### Alternative C: Keep `{:signature}` context projection

Keep `{:signature}` as a context projection populated by the framework, add the Signature field only for extraction.

**Rejected:** `{:signature}` conflates the function name with the type signature. The function name is a framework concept, not a statement property. Making Signature a regular parseable field (`{sig}`) with `HasParser` is more uniform — it follows the same pattern as every other field type.

## Crate impact matrix

| Crate | Impact | Changes |
|-------|--------|---------|
| `kirin-ir` | **Primary** | `HasSignature` returns `Option`, `HasParser` + `PrettyPrint` for `Signature<T>` |
| `kirin-derive-toolkit` | **Primary** | `FieldCategory::Signature`, `FieldData::Signature`, recognition logic |
| `kirin-derive-ir` | **Primary** | Generate `HasSignature` impl from `derive(Dialect)` |
| `kirin-derive-chumsky` | **Primary** | Remove `{:signature}` and `{:return}` projections, whole + projected codegen for `{sig}` / `{sig:inputs}` / `{sig:return}`, remove `manual_parse_emit` and `extract_signature` |
| `kirin-chumsky` | **Secondary** | Function text parser uses `HasSignature` instead of `extract_signature`, remove `extract_signature` from `ParseEmit`, simplify specialize header parsing |
| `kirin-prettyless` | **Secondary** | Remove `signature_text` / `return_type_text` context from `Document`, remove `print_function_signature` / `print_return_types` |
| `kirin-function` | **Migration** | `FunctionBody<T>` gains `sig: Signature<T>` field, format changes to `"fn {:name}{sig} {body}"` |
| `example/toy-qc` | **Migration** | Add `sig: Signature<QubitType>` field, remove manual `HasSignature` impls |
| All test languages | **Migration** | Add `sig: Signature<T>` field to function body variants |

## Migration path

1. Add `FieldCategory::Signature` and recognition to derive-toolkit
2. Implement `HasParser` and `PrettyPrint` for `Signature<T>`
3. Generate `HasSignature` from `derive(Dialect)` — change return to `Option`
4. Implement whole-signature parser codegen (`{sig}` as field reference)
5. Implement projected-signature parser codegen (`{sig:inputs}` + `{sig:return}`)
6. Implement projected-signature printer codegen
7. Remove `{:signature}` and `{:return}` context projections from format string system
8. Remove `extract_signature` from `ParseEmit`, remove `manual_parse_emit`
9. Remove `Document` context state (`signature_text`, `return_type_text`) and helper methods
10. Simplify function text parser: unify specialize paths, strip only `specialize @stage`, extract name from `EmitContext`
11. Migrate `kirin-function`: `FunctionBody<T>` gains `sig` field, format `"fn {:name}{sig} {body}"`
12. Migrate `toy-qc`: add `sig` field, remove manual `HasSignature` impls
13. Migrate test languages: add `sig` field to function body variants

## Resolved questions

1. **Signature field naming:** Recognized by type path, not by name. Any name works (`sig`, `signature`, etc.). Convention: `sig` for brevity.

2. **Signature population:** Either `{sig}` (whole) or `{sig:inputs}` + `{sig:return}` (projected). The two projections must appear together — partial projections are a compile error. No "derived from IR" mode; the Signature is always parsed from text. For graph-based functions, `{sig:inputs}` / `{sig:return}` coexist with `{body:ports}` / `{body:captures}` — they serve different purposes (type signature vs IR structure). Types may appear in both, mirroring MLIR's separation of function type from block argument declarations.

3. **`{:return}` elimination:** `{:return}` context projection is replaced by `{sig:return}` field projection. This makes the projection system uniform: `{field:projection}` for both graph fields and signature fields. The only remaining context projection is `{:name}`.

4. **`{:name}` stays as a framework context projection in format strings:** The function name is NOT a property of the body statement — it's the identity of the abstract function (`Function` → `StagedFunction` → `SpecializedFunction`), which must be a single source of truth across all levels. If dialect developers defined a `name` field on the body statement, it could diverge from the name on the abstract function or staged function, which are higher-level constructs that must stay synchronized. The name flows top-down from the framework (Pipeline → Function → StagedFunction → body printing), so `{:name}` correctly reads from framework context rather than from a field. This is the fundamental difference between name (identity, externally owned) and signature (parsed property, statement-owned). During parsing, `{:name}` consumes the `@symbol` token and stores the name in `EmitContext::function_name` — the function text parser reads it back after `parse_and_emit`. The dialect controls the keyword placement (`fn {:name}`, `gate {:name}`, etc.).

5. **HasSignature on language enums:** `derive(Dialect)` generates it with per-variant dispatch. `#[wraps]` enums delegate to inner type. Non-wraps enums match per variant.

6. **Backward compatibility:** Clean break. Existing manual impls (`CircuitFunction`, `ZXFunction`) are deleted and replaced by derive-generated impls. External code wraps returns in `Some()`.

7. **Function text parser simplification:** The two `specialize` paths (`specialize_with_sig` and `specialize_dialect`) unify into one — the statement parser always handles keyword + name + signature + body via its format string. The function text parser strips `specialize @stage` and passes the rest to `parse_and_emit`. The function name is extracted from `EmitContext::function_name()` (stored by `{:name}` during the statement parse) — no scanning heuristic needed. The two-pass architecture stays (needed for forward references). `stage` declarations still use framework signature parsing (`stage @stage fn @name(T, T) -> T;` syntax is fixed). See "Function text parser simplification" design section.

8. **`HasParser` impl location:** The impl goes in `kirin-chumsky`, same as `HasParser` for `SSAValue` and other `kirin-ir` types. This follows the standard orphan rule pattern — the trait is in `kirin-chumsky`, the type is in `kirin-ir`, the impl goes in the crate that owns the trait.

## Open questions

None — all questions resolved.
