# RFC 0003 Task 7: Dialect-Controlled Function Text Parser

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Change the `specialize` declaration parser so the framework only parses `specialize @stage fn @name`, then delegates the rest (format-controlled parts like ports, yields, body) to the dialect's `ParseEmit` implementation. Add `extract_signature` to `ParseEmit` so the framework can get the specialized signature from the parsed body via `HasSignature`. Support auto-creating staged functions when no prior `stage` declaration exists.

**Architecture:** The two-pass architecture is preserved. Pass 1 still scans for `stage` and `specialize` keywords. Pass 2 changes: for `specialize`, the framework parses `specialize @stage fn @name` prefix, then passes the remaining text (from after `@name` through closing `}`) to `L::parse_and_emit()`. After emit, `L::extract_signature()` provides the specialized signature. If no staged function exists, one is auto-created.

**Key design decisions:**
- Framework still parses `specialize @stage fn @name` — avoids the function-name-extraction problem
- `ParseEmit` gains a default method `extract_signature()` — backward compatible
- `stage` declarations remain framework-controlled and optional
- `Declaration::Specialize` no longer requires a full `Signature` — only stage + function name + body span

---

## Prerequisites

- RFC 0003 Tasks 1-4 complete (format parser, validation, HasSignature, component parsers)
- RFC 0003 Task 5 complete (projection parse codegen) — dialect formats can parse projections
- RFC 0003 Task 8 complete (function name context in printer)
- All tests passing on `rust` branch

## Key files

| File | Role |
|------|------|
| `crates/kirin-chumsky/src/function_text/syntax.rs` | Declaration parser — `specialize_decl`, `body_span`, `Declaration` enum |
| `crates/kirin-chumsky/src/function_text/parse_text.rs` | Two-pass pipeline parsing — `second_pass_concrete`, `apply_specialize_declaration` |
| `crates/kirin-chumsky/src/function_text/dispatch.rs` | `ParseDispatch` trait — blanket impl bounds |
| `crates/kirin-chumsky/src/traits.rs` or `crates/kirin-chumsky/src/traits/parse_emit.rs` | `ParseEmit` trait |
| `crates/kirin-ir/src/signature/has_signature.rs` | `HasSignature` trait (already exists) |

## Background: Current specialize flow

### Pass 1 (headers + indexing)
```
parse_declaration_head(): reads stage/specialize @stage fn @name (4 tokens)
→ first_pass_concrete():
    Declaration::Stage → apply_stage_declaration() → builder.staged_function().signature(sig).new()
    Declaration::Specialize → just record offset, return DeclKeyword::Specialize
```

### Pass 2 (specialize bodies)
```
second_pass_concrete():
    parse_one_declaration::<L>() → Declaration::Specialize { header, body_span }
    body_text = &src[body_span.start..body_span.end]  // inside { }
    apply_specialize_declaration():
        resolve_specialize_target(header.function.name) → (function, staged_function)
        L::parse_and_emit(body_text, &mut emit_ctx) → body_statement
        builder.specialize().staged_func(sf).signature(header.signature).body(body_stmt).new()
```

### Key issue
`header.signature` comes from `fn_signature_parser()` which parses `fn @name(types) -> type`. With dialect-controlled format, the types and return type are parsed by the dialect, not the framework. The framework no longer has the signature.

---

## Task 7.1: Add `extract_signature` to ParseEmit

**File:** Find where `ParseEmit` is defined (likely `crates/kirin-chumsky/src/traits.rs` or a submodule)

- [ ] **Step 0: Find ParseEmit definition**

```bash
grep -rn "pub trait ParseEmit" crates/kirin-chumsky/src/
```

- [ ] **Step 1: Read the current ParseEmit trait**

Understand the current signature and all implementations (derive-generated, blanket, manual).

- [ ] **Step 2: Add `extract_signature` default method**

```rust
pub trait ParseEmit<L: Dialect = Self>: Dialect {
    /// Parse text and emit IR, returning the body statement handle.
    fn parse_and_emit(
        input: &str,
        ctx: &mut EmitContext<'_, L>,
    ) -> Result<Statement, ChumskyError>;

    /// Extract the specialized function signature from a body statement.
    ///
    /// Called by the function text parser after `parse_and_emit` to get the
    /// narrow signature for the `SpecializedFunction`. Returns `None` by default,
    /// which means the framework uses its own parsed signature (backward compat).
    ///
    /// Dialect authors implement this when their function body type implements
    /// `HasSignature<L>`. The implementation reads the statement's definition
    /// from the stage and delegates to `HasSignature::signature()`.
    fn extract_signature(
        _stmt: &Statement,
        _stage: &StageInfo<L>,
    ) -> Option<Signature<L::Type>> {
        None
    }
}
```

This is backward compatible — existing `ParseEmit` impls don't need to change.

- [ ] **Step 3: Build and verify**

```bash
cargo build -p kirin-chumsky
cargo nextest run -p kirin-chumsky
```

- [ ] **Step 4: Commit**

```
feat(chumsky): add extract_signature default method to ParseEmit trait
```

---

## Task 7.2: Simplify Declaration::Specialize

**File:** `crates/kirin-chumsky/src/function_text/syntax.rs`

Currently `Declaration::Specialize` stores a full `Header` with signature. We need to make the signature optional since the dialect may provide it instead.

- [ ] **Step 1: Read the current Declaration enum and specialize_decl parser**

- [ ] **Step 2: Change Declaration::Specialize to not require signature**

```rust
#[derive(Debug, Clone)]
pub(super) enum Declaration<'src, T> {
    Stage(Header<'src, T>),
    Specialize {
        stage: SymbolName<'src>,
        function: SymbolName<'src>,
        /// Optional signature from framework-parsed `fn @name(types) -> type`.
        /// None when dialect controls the format.
        signature: Option<Signature<T>>,
        /// Span of the body portion (brace-balanced region including prefix).
        body_span: SimpleSpan,
        /// Span of the entire specialize declaration.
        span: SimpleSpan,
    },
}
```

- [ ] **Step 3: Update specialize_decl parser**

Change from parsing `fn_signature_parser` to parsing `fn @name` then scanning the rest:

```rust
let specialize_decl = identifier("specialize")
    .ignore_then(symbol())                       // @stage
    .then_ignore(identifier("fn"))               // fn keyword
    .then(symbol())                              // @name
    .then(body_span::<I>())                      // everything through closing }
    .map_with(|((stage, function), body_span), extra| Declaration::Specialize {
        stage,
        function,
        signature: None,     // Dialect will provide via HasSignature
        body_span,
        span: extra.span(),
    });
```

**Wait — `body_span` currently starts scanning from the current position and finds the first `{`.** With the new approach, `body_span` needs to capture everything from after `@name` (including `(types) -> type {`) to the closing `}`.

Actually, `body_span` already does this! It skips tokens until `{`, then tracks brace depth. The span covers from the first skipped token through `}`. So this already captures the format-controlled area.

The `body_text` in pass 2 will now include the entire format-controlled area: `(%q0: Qubit) -> Qubit { ... }`, not just `{ ... }`.

- [ ] **Step 4: Keep the old specialize parser as fallback**

For backward compatibility, try the new parser first. If the text has `fn @name(types) -> type { body }` with a parseable signature, extract it. Otherwise, use signature = None.

Actually, simpler: always use `signature: None` for the specialize parser. The framework will check `L::extract_signature()` in pass 2. If it returns `None`, fall back to the legacy behavior (parse `fn @name(types) -> type` ourselves).

But this changes the Declaration type for ALL dialects. Let me think...

**REVISED APPROACH:** Keep both old and new specialize parsers. Try the new one (no signature) first; if the dialect provides `extract_signature`, use it. If not, fall back to parsing the signature ourselves.

Actually, the simplest approach: **keep the existing parser that extracts the signature, but make `signature` an `Option`.**

```rust
let specialize_decl = identifier("specialize")
    .ignore_then(symbol())
    .then(fn_signature_parser::<I, L>().or_not())  // Try to parse signature, optional
    .then(body_span::<I>())
    .map_with(|((stage, sig), body_span), extra| {
        Declaration::Specialize {
            stage,
            function: sig.as_ref().map(|s| s.function).unwrap_or_else(|| ???),
            signature: sig.map(|s| s.signature),
            body_span,
            span: extra.span(),
        }
    });
```

Hmm, this gets awkward because we ALWAYS need the function name. Let me separate the concerns:

```rust
let specialize_decl = identifier("specialize")
    .ignore_then(symbol())                          // @stage
    .then(
        // Always parse fn @name; signature is optional
        identifier("fn")
            .ignore_then(symbol())                  // @name — always needed
            .then(
                // Try to parse (types) -> type — optional for new dialects
                fn_params_and_return::<I, L>().or_not()
            )
    )
    .then(body_span::<I>())
    .map_with(|((stage, (function, sig)), body_span), extra| {
        Declaration::Specialize {
            stage,
            function,
            signature: sig,
            body_span,
            span: extra.span(),
        }
    });
```

Where `fn_params_and_return` parses just `(types) -> type` without `fn @name`.

- [ ] **Step 5: Extract `fn_params_and_return` parser**

Split `fn_signature_parser` into two parts:
1. `fn @name` — always parsed
2. `(types) -> type` — optionally parsed

```rust
fn fn_params_and_return_parser<'src, I, L>()
-> impl Parser<'src, I, Signature<L::Type>, ParserError<'src>>
where
    I: TokenInput<'src>,
    L: Dialect + HasParser<'src>,
    L::Type: HasParser<'src, Output = L::Type>,
{
    type_list_parser::<I, L>()
        .then_ignore(just(Token::Arrow))
        .then(L::Type::parser())
        .map(|(params, ret)| Signature::new(params, ret, ()))
}
```

- [ ] **Step 6: Build and test**

```bash
cargo build -p kirin-chumsky
cargo nextest run --workspace
```

All existing tests must still pass since the signature is `Some(...)` for all current dialects.

- [ ] **Step 7: Commit**

```
feat(chumsky): make specialize signature optional in Declaration
```

---

## Task 7.3: Update body_span to capture format-controlled area

**File:** `crates/kirin-chumsky/src/function_text/syntax.rs`

Currently, `body_span` captures from the first token (e.g., `digraph`) through the closing `}`. When signature is `None`, the body_text needs to include the entire format area (after `fn @name`).

- [ ] **Step 1: Verify body_span behavior**

Check: when `signature` is `None` and the text is `specialize @A fn @foo(%q: Qubit) -> Qubit { ... }`, does `body_span` capture `(%q: Qubit) -> Qubit { ... }` (everything after the `fn @name` prefix)?

The `body_span` parser starts at whatever position it's called from. After parsing `specialize @stage fn @name`, the parser position is right after `@name`. Then `body_span` scans forward until `{` and through matching `}`. So `body_span` captures everything from `(` (the token after `@name`) through `}`. ✅

- [ ] **Step 2: Update body_text extraction in second_pass_concrete**

Currently:
```rust
let body_text = &ctx.src[body_span.start..body_span.end];
```

This extracts the body_text. When signature is `Some`, body_text is `{ ... }` (inside braces). When signature is `None`, body_text is `(%q: Qubit) -> Qubit { ... }` (entire format area).

We need different body_text extraction depending on whether signature was parsed:
- With signature: `body_text` = inside `{ }` (current behavior, dialect parses `{body}`)
- Without signature: `body_text` = everything after `fn @name` (dialect parses `fn {:name}({body:ports}) -> ...`)

Wait — when signature is `None`, the dialect's format string handles everything including `fn {:name}(...)`. But the framework already consumed `fn @name` from the token stream. So the dialect's `{function:name}` projection won't see `@name` in the body_text.

**ISSUE:** If the dialect format is `fn {function:name}({body:ports}) -> ...`, and the framework already parsed `fn @name`, the body_text starts at `(` — the dialect's parser won't see `fn @name`.

**SOLUTION:** When signature is `None`, include `fn @name` in the body_text. Change the body_span to cover from `fn` onward (not just from after `@name`).

Or better: the framework passes the function name as context, and the dialect's format parser for `{function:name}` reads it from context rather than parsing it from text.

**SIMPLEST SOLUTION:** Don't include `fn @name` in body_text. The `{function:name}` is handled by the Document's function_name context (Task 8). For parsing, skip `{function:name}` entirely — it's a print-only projection. The parser gets the function name from the framework prefix.

This means in the format string `fn {function:name}({body:ports}) -> {body:yields} { {body:body} }`:
- For printing: `{function:name}` reads from `doc.function_name()` ✅ (Task 8)
- For parsing: `{function:name}` parses `@symbol` from the text
- But the framework already parsed `@name`...

**REVISED SIMPLEST SOLUTION:** Keep parsing `fn @name` at the framework level AND in the dialect. The dialect re-parses `fn @name` from body_text. Body_text includes `fn @name(...)` — everything after `specialize @stage`. The framework extracts the function name for lookup but the dialect also parses it.

Let me adjust:

- [ ] **Step 3: Change body_span capture point**

After `specialize @stage`, capture EVERYTHING (including `fn @name(types) -> type { body }`) as body_span:

```rust
let specialize_decl = identifier("specialize")
    .ignore_then(symbol())                          // @stage
    .then(
        // Peek at fn @name for framework lookup, but don't consume
        identifier("fn").ignore_then(symbol())
    )
    .then(body_span_from_current::<I>())            // Everything from fn through }
    .map_with(...)
```

Actually, we can't "peek" with chumsky easily. Alternative: extract function name from body_text after the fact:

```rust
let specialize_decl = identifier("specialize")
    .ignore_then(symbol())                          // @stage
    .then(full_body_span::<I>())                    // Everything from fn through }
    .map_with(|(stage, body_span), extra| {
        // Extract function name from body_span tokens
        Declaration::Specialize {
            stage,
            body_span,
            span: extra.span(),
        }
    });
```

Then in `parse_declaration_head()` or `second_pass_concrete()`, extract the function name from the body tokens manually.

**Actually, the current `parse_declaration_head()` already reads `specialize @stage fn @name` from raw tokens.** It doesn't use the chumsky parser for this — it directly reads `tokens[start_index]`, `tokens[start_index+1]`, etc. So we can keep that logic for function name extraction.

Let me re-think the approach:

**FINAL APPROACH:**
1. `parse_declaration_head()` reads `specialize @stage fn @name` (4 tokens) as today — framework extracts function name
2. `body_span` starts AFTER `fn @name` — captures everything from `(` through `}`
3. Body_text = `(types) -> type { ... }` when old format, or `(%q: Qubit) -> Qubit { ... }` when new format
4. Dialect's format string is `({body:ports}) -> {body:yields} { {body:body} }` (without `fn {function:name}`)
5. For printing, the framework prepends `specialize @stage fn @name` and the dialect prints the rest

Wait, but the RFC says the dialect format is `fn {function:name}({body:ports}) -> ...`. If the framework already handles `fn @name`, the dialect format should be `({body:ports}) -> {body:yields} { {body:body} }`.

**This simplifies everything.** The framework handles `specialize @stage fn @name` prefix. The dialect format handles the body-specific parts: ports, yields, body.

- [ ] **Step 4: Decide on body_text boundaries**

For the **current format** (`{body}` = full digraph/region): body_text = inside `{ }` (current behavior)

For the **new format** (projections): body_text = from `(` after `@name` through closing `}`. This includes the format-controlled parts.

We need to distinguish these cases. The simplest way: always pass the full body_text (from after `fn @name` through `}`). Let the dialect's `parse_and_emit` handle parsing the relevant parts.

But current dialects (using `{body}`) expect body_text = just the brace-enclosed body. Changing body_text boundaries would break them.

**BACKWARD-COMPATIBLE APPROACH:**
- When `signature` is `Some` (framework parsed `(types) -> type`): body_text = inside `{ }` (current)
- When `signature` is `None` (dialect controls): body_text = after `fn @name` through `}`

This is determined by whether `fn_params_and_return_parser` succeeded.

- [ ] **Step 5: Commit plan adjustment (no code yet)**

Document the body_text boundary decision for the implementation.

---

## Task 7.4: Update second_pass_concrete for dialect-controlled signature

**File:** `crates/kirin-chumsky/src/function_text/parse_text.rs`

- [ ] **Step 1: Read current `second_pass_concrete` and `apply_specialize_declaration`**

- [ ] **Step 2: Update `apply_specialize_declaration` to handle optional signature**

```rust
fn apply_specialize_declaration<L>(
    stage: &mut StageInfo<L>,
    stage_id: CompileStage,
    function_name: &SymbolName<'_>,     // Always available from parse_declaration_head
    framework_signature: Option<&Signature<L::Type>>,  // None when dialect controls
    body_text: &str,
    span: SimpleSpan,
    function_lookup: &FxHashMap<String, Function>,
    staged_lookup: &FxHashMap<StagedKey, StagedFunction>,
    state: &mut ParseState,
) -> Result<(), FunctionParseError>
where
    L: Dialect + ParseEmit<L>,
{
    // Resolve target staged function
    let (function, staged_function) =
        resolve_specialize_target::<L>(stage_id, function_name, span, function_lookup, staged_lookup)?;

    stage.with_builder(|builder| {
        let body_statement = {
            let mut emit_ctx = EmitContext::new(builder);
            L::parse_and_emit(body_text, &mut emit_ctx).map_err(|err| ...)?
        };

        // Determine signature: dialect-provided or framework-parsed
        let signature = if let Some(sig) = framework_signature {
            sig.clone()
        } else if let Some(sig) = L::extract_signature(&body_statement, stage) {
            sig
        } else {
            return Err(FunctionParseError::new(
                FunctionParseErrorKind::EmitFailed,
                Some(span),
                "dialect does not provide function signature (implement extract_signature on ParseEmit)",
            ));
        };

        builder
            .specialize()
            .staged_func(staged_function)
            .signature(signature)
            .body(body_statement)
            .new()
            .map_err(|err| ...)?;

        Ok(())
    })?;

    state.record(function);
    Ok(())
}
```

- [ ] **Step 3: Update `second_pass_concrete` to pass optional signature**

```rust
let Declaration::Specialize {
    stage: _,
    function,
    signature,      // Option<Signature<L::Type>>
    body_span,
    span,
} = declaration else { ... };

// Determine body_text based on whether we have a framework signature
let body_text = if signature.is_some() {
    // Old behavior: body_text is inside { }
    &ctx.src[body_span.start..body_span.end]
} else {
    // New behavior: body_text includes format area after fn @name
    &ctx.src[body_span.start..body_span.end]
    // (body_span already covers the right range based on how we parsed it)
};

apply_specialize_declaration::<L>(
    stage,
    stage_id,
    &function,              // SymbolName from parse_declaration_head
    signature.as_ref(),     // None when dialect controls
    body_text,
    span,
    ctx.function_lookup,
    ctx.staged_lookup,
    ctx.state,
)?;
```

- [ ] **Step 4: Build and test**

```bash
cargo build --workspace
cargo nextest run --workspace
```

All existing tests must pass — `signature` is always `Some(...)` for current dialects.

- [ ] **Step 5: Commit**

```
feat(chumsky): support dialect-provided signature in specialize declarations
```

---

## Task 7.5: Auto-create staged function from specialize

**File:** `crates/kirin-chumsky/src/function_text/parse_text.rs`

When no `stage` declaration exists for a function, and `extract_signature` provides a signature, auto-create the staged function.

- [ ] **Step 1: Update `resolve_specialize_target` to handle missing staged function**

Currently returns `MissingStageDeclaration` error. Add a fallback path:

```rust
fn resolve_or_create_specialize_target<L>(
    stage: &mut StageInfo<L>,
    stage_id: CompileStage,
    function_name: &SymbolName<'_>,
    signature: &Signature<L::Type>,
    function_lookup: &mut FxHashMap<String, Function>,
    staged_lookup: &mut FxHashMap<StagedKey, StagedFunction>,
    pipeline: &mut Pipeline<impl StageMeta>,
) -> Result<(Function, StagedFunction), FunctionParseError>
where
    L: Dialect,
    L::Type: kirin_ir::Placeholder,
{
    // Get or create function
    let function = get_or_create_function_by_name(pipeline, function_lookup, function_name.name);
    let fn_symbol = fn_symbol(pipeline, function);

    let key = StagedKey { stage: stage_id, function };
    if let Some(staged) = staged_lookup.get(&key).copied() {
        return Ok((function, staged));
    }

    // Auto-create staged function with the extracted signature
    let staged = stage.with_builder(|builder| {
        builder
            .staged_function()
            .name(fn_symbol)
            .signature(signature.clone())
            .new()
    }).map_err(|err| FunctionParseError::new(
        FunctionParseErrorKind::EmitFailed,
        None,
        format!("failed to auto-create staged function: {}", err),
    ))?;

    staged_lookup.insert(key, staged);
    Ok((function, staged))
}
```

- [ ] **Step 2: Thread Pipeline access into pass 2**

The `SecondPassCtx` currently doesn't hold a mutable pipeline reference. We need to add it or restructure the pass 2 loop.

Check: does `stage.with_builder()` need a `Pipeline` reference, or does `StageInfo` have its own builder?

Read `StageInfo::with_builder` to understand what it needs. If it's self-contained, we don't need Pipeline access in pass 2.

- [ ] **Step 3: Wire auto-creation into apply_specialize_declaration**

When `signature` is `None` (dialect controls) AND `resolve_specialize_target` fails (no staged function), use `resolve_or_create_specialize_target` with the dialect-extracted signature.

- [ ] **Step 4: Build and test**

```bash
cargo nextest run --workspace
```

- [ ] **Step 5: Commit**

```
feat(chumsky): auto-create staged function from dialect-extracted signature
```

---

## Task 7.6: Implement extract_signature for toy-qc

**File:** `example/toy-qc/src/circuit.rs`, `example/toy-qc/src/zx.rs`

- [ ] **Step 1: Read where ParseEmit is implemented for Circuit and ZX**

These are likely derive-generated. Check if the derive generates a `extract_signature` override.

If the derive doesn't generate it, implement it manually:

```rust
impl ParseEmit<Circuit> for Circuit {
    fn parse_and_emit(input: &str, ctx: &mut EmitContext<'_, Circuit>) -> Result<Statement, ChumskyError> {
        // derive-generated
    }

    fn extract_signature(stmt: &Statement, stage: &StageInfo<Circuit>) -> Option<Signature<QubitType>> {
        // Check if the statement is a CircuitFunction body
        let def = stmt.expect_info(stage).definition();
        if let Some(cf) = def.downcast_ref::<CircuitFunction>() {
            Some(cf.signature(stage))
        } else {
            None
        }
    }
}
```

Wait — can we downcast a dialect statement? Check how `definition()` returns the definition and what type it is.

- [ ] **Step 2: Investigate statement downcast mechanism**

Read how `stmt.expect_info(stage).definition()` works and what `&dyn Dialect` looks like. If we can't downcast, we need another approach (e.g., `HasSignature` as a trait object).

- [ ] **Step 3: Implement extract_signature**

Based on investigation, implement the appropriate approach.

- [ ] **Step 4: Test**

```bash
cargo nextest run -p toy-qc
```

- [ ] **Step 5: Commit**

```
feat(toy-qc): implement extract_signature for Circuit and ZX dialects
```

---

## Task 7.7: End-to-end test with dialect-controlled format

- [ ] **Step 1: Write a test that uses `specialize` without `stage`**

In toy-qc's e2e tests, add a test that parses a program with only `specialize` and no `stage`:

```rust
#[test]
fn test_specialize_without_stage() {
    let src = r#"
specialize @circuit fn @test_gate(%q: Qubit) -> Qubit {
    digraph ^dg0(%q: Qubit) {
        %q2 = h %q -> Qubit;
        yield %q2;
    }
}
"#;
    let mut pipeline = create_pipeline();
    pipeline.parse(src).expect("should parse without stage declaration");
}
```

- [ ] **Step 2: Verify roundtrip**

Parse → print → re-parse → compare.

- [ ] **Step 3: Commit**

```
test(toy-qc): add end-to-end test for specialize without stage declaration
```

---

## Dependency Graph

```
Task 7.1 (extract_signature) ── Task 7.2 (Declaration changes) ── Task 7.3 (body_text boundaries)
                                                                          │
                                                                   Task 7.4 (second_pass update)
                                                                          │
                                                                   Task 7.5 (auto-create staged fn)
                                                                          │
                                                                   Task 7.6 (toy-qc impls)
                                                                          │
                                                                   Task 7.7 (e2e tests)
```

## Risk Assessment

**HIGH RISK:** Task 7.4 (second_pass update). Changes the core function-parsing flow. Must maintain backward compatibility — existing dialects with `{body}` format must work unchanged.

**MEDIUM RISK:** Task 7.5 (auto-create staged function). Threading mutable pipeline access into pass 2 may require restructuring `SecondPassCtx`. The `StageInfo::with_builder` API may or may not need external Pipeline state.

**MEDIUM RISK:** Task 7.3 (body_text boundaries). Getting the right text span for dialect vs framework parsing is fiddly. Off-by-one in span extraction breaks everything.

**LOW RISK:** Tasks 7.1 (default method), 7.2 (Declaration refactor), 7.6 (toy-qc impls), 7.7 (tests).

## Open questions for implementer

1. **Can we downcast dialect statements?** `stmt.expect_info(stage).definition()` returns `&dyn L` — can we call `HasSignature::signature()` on it? Check the `Definition` type and `Any` bounds.

2. **Does `StageInfo::with_builder` need Pipeline access?** If yes, we need to restructure `SecondPassCtx` to hold `&mut Pipeline<S>`. If no (builder is self-contained), Task 7.5 is simpler.

3. **Body_text for new format:** When the dialect format is `({body:ports}) -> {body:yields} { {body:body} }`, body_text must start at `(` (after `fn @name`). Verify that `body_span` from the revised parser captures this correctly.
