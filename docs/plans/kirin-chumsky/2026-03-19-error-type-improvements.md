# Error Type Improvements: Unified Hierarchy and EmitError Provenance

## Problem

Two related issues in `kirin-chumsky`'s error handling:

### 1. `parse_and_emit` conflates EmitError with ParseError

In `crates/kirin-chumsky/src/traits/parse_emit.rs:44-49`, the blanket `ParseEmit` impl for `SimpleParseEmit` converts `EmitError` into `ParseError` with a zero span:

```rust
ast.emit(ctx).map_err(|e| {
    vec![ParseError {
        message: e.to_string(),
        span: chumsky::span::SimpleSpan::from(0..0),
    }]
})
```

This loses the `EmitError` variant information (was it `UndefinedSSA`, `UndefinedBlock`, or `Custom`?) and produces a meaningless `0..0` span. Downstream code receiving `Vec<ParseError>` cannot distinguish parse failures from emit failures.

### 2. Three error types without unified hierarchy

The crate has three distinct error types that are not connected:

- **`ParseError`** (`traits/has_parser.rs:90`): `{ message: String, span: SimpleSpan }` -- flat struct, no variants.
- **`EmitError`** (`traits/emit_ir.rs:6`): `UndefinedSSA(String) | UndefinedBlock(String) | Custom(String)` -- structured enum.
- **`FunctionParseError`** (`function_text/error.rs:35`): `{ kind: FunctionParseErrorKind, span: Option<SimpleSpan>, message: String, source: Option<Box<dyn Error>> }` -- rich error with kind enum and error chain.

There is no common trait, enum wrapper, or conversion path between these. `FunctionParseError` can wrap `ParseError` and `EmitError` via its `source` field, but this is ad-hoc.

## Research Findings

### `ParseError` structure

```rust
pub struct ParseError {
    pub message: String,
    pub span: SimpleSpan,
}
```

Produced by chumsky's error recovery. The span is always populated (chumsky provides it). Used as `Vec<ParseError>` throughout the API.

### `EmitError` structure

```rust
pub enum EmitError {
    UndefinedSSA(String),
    UndefinedBlock(String),
    Custom(String),
}
```

Produced during IR emission from AST. Has no span information because the emit phase works with AST nodes that have already lost their spans (the AST types store `Spanned` wrappers but `EmitIR::emit` does not propagate spans into errors).

### `FunctionParseError` structure

Has 6 kinds: `InvalidHeader`, `UnknownStage`, `InconsistentFunctionName`, `MissingStageDeclaration`, `BodyParseFailed`, `EmitFailed`. The `BodyParseFailed` and `EmitFailed` kinds already distinguish parse vs. emit failures at the function level.

### Error flow

```
chumsky parser -> Vec<ParseError>
                        |
                        v
         parse_and_emit() -> Vec<ParseError>  (EmitError squashed into ParseError)
                        |
                        v
ParseStatementText / ParsePipelineText -> Vec<ParseError> or FunctionParseError
```

The `FunctionParseError` wraps lower-level errors via `with_source()`, but the `EmitError` information is already lost by the time it reaches `FunctionParseError`.

## Proposed Design

### Option A: Tagged ParseError (minimal change)

Add a `kind` field to `ParseError`:

```rust
#[derive(Debug, Clone)]
pub enum ParseErrorKind {
    /// Error from the chumsky parser.
    Syntax,
    /// Error during IR emission (e.g., undefined SSA, undefined block).
    Emit(EmitErrorKind),
}

#[derive(Debug, Clone)]
pub enum EmitErrorKind {
    UndefinedSSA,
    UndefinedBlock,
    Custom,
}

pub struct ParseError {
    pub kind: ParseErrorKind,
    pub message: String,
    pub span: SimpleSpan,
}
```

The `parse_and_emit` conversion becomes:
```rust
ast.emit(ctx).map_err(|e| {
    vec![ParseError {
        kind: ParseErrorKind::Emit(match &e {
            EmitError::UndefinedSSA(_) => EmitErrorKind::UndefinedSSA,
            EmitError::UndefinedBlock(_) => EmitErrorKind::UndefinedBlock,
            EmitError::Custom(_) => EmitErrorKind::Custom,
        }),
        message: e.to_string(),
        span: SimpleSpan::from(0..0), // still no span, but kind is preserved
    }]
})
```

**Pros:** Backward-compatible (existing code ignoring `kind` still works). Preserves error provenance.
**Cons:** Span is still `0..0` for emit errors.

### Option B: Sum type replacing Vec<ParseError> (larger change)

Replace `Vec<ParseError>` with a dedicated error type:

```rust
pub enum ChumskyError {
    Parse(Vec<ParseError>),
    Emit(EmitError),
}
```

The `ParseEmit::parse_and_emit` return type changes from `Result<Statement, Vec<ParseError>>` to `Result<Statement, ChumskyError>`.

**Pros:** Clean separation. No zero-span hack.
**Cons:** Breaking change to `ParseEmit` trait signature. All callers must be updated.

### Recommendation: Option B (user decision)

Option B was chosen for a cleaner separation. The `ParseEmit` trait return type changes from `Result<Statement, Vec<ParseError>>` to `Result<Statement, ChumskyError>`. This is a breaking change but provides proper separation between parse and emit failures without the zero-span hack.

### Span improvement (future work)

To fix the `0..0` span, `EmitError` would need to carry an optional span:
```rust
pub enum EmitError {
    UndefinedSSA { name: String, span: Option<SimpleSpan> },
    UndefinedBlock { name: String, span: Option<SimpleSpan> },
    Custom { message: String, span: Option<SimpleSpan> },
}
```

This requires changes to `EmitIR::emit` and all emit implementations. Defer to a follow-up.

## Implementation Steps

1. Add `ParseErrorKind` and `EmitErrorKind` enums in `traits/has_parser.rs` (alongside `ParseError`).
2. Add `kind: ParseErrorKind` field to `ParseError` with default `Syntax`.
3. Update the `From<Rich<'_, Token<'_>>>` conversion (if any) to set `kind: Syntax`.
4. Update `parse_and_emit` in `parse_emit.rs` to produce `ParseError` with `Emit(...)` kind.
5. Update `FunctionParseError` conversion paths to preserve the `kind` information.
6. Grep for all `ParseError { message, span }` struct literals and add `kind: ParseErrorKind::Syntax`.

## Risk Assessment

**Medium risk.** Adding a field to `ParseError` is technically a breaking change if anyone constructs it with struct literal syntax (no `..Default::default()`). Within this workspace, `ParseError` is constructed in:
- `parse_emit.rs` (the fix site)
- chumsky error conversion code
- test code

All internal sites need updating. External users constructing `ParseError` directly would also need to add the `kind` field.

Mitigation: make `kind` default to `Syntax` by deriving `Default` on `ParseErrorKind` or providing a constructor method.

## Testing Strategy

- Update existing tests that construct `ParseError` to include `kind`.
- Add a test in `parse_emit.rs` that verifies an `EmitError::UndefinedSSA` is converted to a `ParseError` with `kind: Emit(UndefinedSSA)`.
- Run all roundtrip tests to verify no behavioral regressions.
- Test that `FunctionParseError` with `kind: BodyParseFailed` and `kind: EmitFailed` correctly preserves the inner `ParseErrorKind`.
