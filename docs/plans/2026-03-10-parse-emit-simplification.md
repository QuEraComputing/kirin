# ParseEmit Simplification Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace `HasParserEmitIR` and `HasDialectEmitIR` witness traits with a single `ParseEmit<L>` trait that internalizes the text lifetime, eliminating GAT projection bounds and giving downstream developers a clean manual implementation path.

**Architecture:** New `ParseEmit<L>` trait combines parse+emit into one `&str -> Result<Statement>` method. A `SimpleParseEmit` marker trait provides a blanket impl for non-recursive dialects. The pipeline parsing path (`second_pass_concrete`) is restructured to pass source text through `SecondPassCtx` and use `ParseEmit::parse_and_emit` instead of `L::emit_parsed`. Derive codegen generates `ParseEmit` instead of the two witness traits.

**Tech Stack:** Rust traits, proc-macro codegen (syn/quote), kirin-chumsky, kirin-derive-chumsky

---

### Task 1: Define `ParseEmit` and `SimpleParseEmit` traits

**Files:**
- Create: `crates/kirin-chumsky/src/traits/parse_emit.rs`
- Modify: `crates/kirin-chumsky/src/traits/mod.rs:3-26`
- Modify: `crates/kirin-chumsky/src/lib.rs:83-86` (prelude re-exports)

**Step 1: Create the new trait file**

Create `crates/kirin-chumsky/src/traits/parse_emit.rs`:

```rust
use kirin_ir::{Dialect, Statement};

use super::{EmitContext, EmitError, EmitIR, HasParser, ParseError, parse_ast};

/// A dialect that can parse text and emit IR in one step.
///
/// This replaces the old `HasParserEmitIR` + `HasDialectEmitIR` witness traits.
/// Downstream developers implement this trait to plug into `ParseStatementText`
/// and `ParsePipelineText` without needing `#[derive(HasParser)]`.
///
/// # Three implementation paths
///
/// 1. **Derive**: `#[derive(HasParser)]` generates this automatically.
/// 2. **Marker**: Implement `SimpleParseEmit` for non-recursive dialects
///    (no `Block`/`Region` fields) to get a blanket impl for free.
/// 3. **Manual**: Implement directly for full control over parse+emit.
pub trait ParseEmit<L: Dialect = Self>: Dialect {
    /// Parse input text and emit a single IR statement.
    fn parse_and_emit(
        input: &str,
        ctx: &mut EmitContext<'_, L>,
    ) -> Result<Statement, Vec<ParseError>>;
}

/// Marker trait for dialects whose `HasParser::Output` directly implements `EmitIR`.
///
/// Provides a blanket `ParseEmit` impl. Only works for non-recursive dialects
/// (no `Block`/`Region` fields) — recursive types cause E0275 due to the
/// `for<'t> <L as HasParser<'t>>::Output: EmitIR<L>` bound.
pub trait SimpleParseEmit: Dialect {}

impl<L> ParseEmit<L> for L
where
    L: SimpleParseEmit,
    for<'t> L: HasParser<'t>,
    for<'t> <L as HasParser<'t>>::Output: EmitIR<L, Output = Statement>,
{
    fn parse_and_emit(
        input: &str,
        ctx: &mut EmitContext<'_, L>,
    ) -> Result<Statement, Vec<ParseError>> {
        let ast = parse_ast::<L>(input)?;
        ast.emit(ctx).map_err(|e| {
            vec![ParseError {
                message: e.to_string(),
                span: chumsky::span::SimpleSpan::from(0..0),
            }]
        })
    }
}
```

**Step 2: Wire up module and re-exports**

In `crates/kirin-chumsky/src/traits/mod.rs`, add:
```rust
mod parse_emit;
```
And update the `pub use` to include:
```rust
pub use parse_emit::*;
```

In `crates/kirin-chumsky/src/lib.rs` prelude, replace `HasParserEmitIR` with `ParseEmit, SimpleParseEmit`.

**Step 3: Run `cargo build -p kirin-chumsky`**

Expected: Compiles (new traits exist alongside old ones temporarily).

**Step 4: Commit**

```
feat(chumsky): add ParseEmit and SimpleParseEmit traits
```

---

### Task 2: Update `ParseStatementText` to use `ParseEmit`

**Files:**
- Modify: `crates/kirin-chumsky/src/traits/parse_text.rs:72-134`

**Step 1: Change `parse_statement_on_stage` bounds**

Replace `for<'t> L: HasParserEmitIR<'t>` with `L: ParseEmit<L>`:

```rust
fn parse_statement_on_stage<L>(
    stage: &mut StageInfo<L>,
    input: &str,
) -> Result<kirin_ir::Statement, Vec<ParseError>>
where
    L: Dialect + ParseEmit<L>,
{
    let existing_ssas = collect_existing_ssas(stage);
    let mut emit_ctx = EmitContext::new(stage);
    for (name, ssa) in existing_ssas {
        emit_ctx.register_ssa(name, ssa);
    }
    L::parse_and_emit(input, &mut emit_ctx)
}
```

Note: `parse_ast` + `emit_parsed` is replaced by single `parse_and_emit` call.

**Step 2: Update both `ParseStatementText` impl bounds**

For `StageInfo<L>` (line 94-106): change `for<'t> L: HasParserEmitIR<'t>` to `L: ParseEmit<L>`.

For `Pipeline<S>` (line 108-134): change `for<'t> L: HasParserEmitIR<'t>` to `L: ParseEmit<L>`.

**Step 3: Run `cargo build -p kirin-chumsky`**

Expected: Fails because nothing implements `ParseEmit` yet (old impls use `HasParserEmitIR`). This is expected — we'll fix it in later tasks.

**Step 4: Commit**

```
refactor(chumsky): update ParseStatementText to use ParseEmit
```

---

### Task 3: Update `ParseDispatch` and pipeline helpers to use `ParseEmit`

**Files:**
- Modify: `crates/kirin-chumsky/src/function_text/dispatch.rs:16,48-70`
- Modify: `crates/kirin-chumsky/src/function_text/parse_text.rs:92,188-194,252-288,548-592`

**Step 1: Add `src` field to `SecondPassCtx`**

In `crates/kirin-chumsky/src/function_text/parse_text.rs`, add `src: &'t str` to `SecondPassCtx`:

```rust
pub struct SecondPassCtx<'t> {
    pub tokens: &'t [(Token<'t>, SimpleSpan)],
    pub start_index: usize,
    pub src: &'t str,
    pub function_lookup: &'t FxHashMap<String, Function>,
    pub staged_lookup: &'t FxHashMap<StagedKey, StagedFunction>,
    pub state: &'t mut ParseState,
}
```

Update the `SecondPassCtx` construction site in `Pipeline::parse()` (around line 376) to pass `src`:

```rust
let mut ctx = SecondPassCtx {
    tokens: &tokens,
    start_index,
    src,
    function_lookup: &function_lookup,
    staged_lookup: &staged_lookup,
    state: &mut state,
};
```

**Step 2: Restructure `apply_specialize_declaration`**

Change signature to take `body_text: &str` instead of `body: &<L as HasParser<'src>>::Output`:

```rust
fn apply_specialize_declaration<L>(
    stage: &mut StageInfo<L>,
    stage_id: CompileStage,
    header: &Header<'_, L::Type>,
    body_text: &str,
    span: SimpleSpan,
    function_lookup: &FxHashMap<String, Function>,
    staged_lookup: &FxHashMap<StagedKey, StagedFunction>,
    state: &mut ParseState,
) -> Result<(), FunctionParseError>
where
    L: Dialect + ParseEmit<L>,
{
    let (function, staged_function) =
        resolve_specialize_target::<L>(stage_id, header, span, function_lookup, staged_lookup)?;

    let body_statement = {
        let mut emit_ctx = EmitContext::new(stage);
        L::parse_and_emit(body_text, &mut emit_ctx).map_err(|errs| {
            let message = errs.iter().map(|e| e.message.as_str()).collect::<Vec<_>>().join("; ");
            FunctionParseError::new(
                FunctionParseErrorKind::EmitFailed,
                Some(span),
                message,
            )
        })?
    };

    stage
        .specialize()
        .staged_func(staged_function)
        .signature(header.signature.clone())
        .body(body_statement)
        .new()
        .map_err(|err| {
            FunctionParseError::new(
                FunctionParseErrorKind::EmitFailed,
                Some(span),
                err.to_string(),
            )
        })?;

    state.record(function);
    Ok(())
}
```

**Step 3: Restructure `second_pass_concrete`**

Change to extract body text from source using span, then call `apply_specialize_declaration` with text:

```rust
pub fn second_pass_concrete<'t, L>(
    stage: &mut StageInfo<L>,
    stage_id: CompileStage,
    ctx: &mut SecondPassCtx<'t>,
) -> Result<usize, FunctionParseError>
where
    L: Dialect + ParseEmit<L> + HasParser<'t>,
    L::Type: HasParser<'t, Output = L::Type>,
{
    let (declaration, consumed_span) = parse_one_declaration::<L>(&ctx.tokens[ctx.start_index..])
        .map_err(parse_error_from_chumsky)?;
    let next_index = advance_to_next_declaration(ctx.tokens, ctx.start_index, consumed_span);

    let Declaration::Specialize { header, body: _, span } = declaration else {
        return Err(FunctionParseError::new(
            FunctionParseErrorKind::InvalidHeader,
            Some(consumed_span),
            "expected specialize declaration",
        ));
    };

    // Extract body text from source using the span
    let body_text = &ctx.src[span.start..span.end];

    apply_specialize_declaration::<L>(
        stage,
        stage_id,
        &header,
        body_text,
        span,
        ctx.function_lookup,
        ctx.staged_lookup,
        ctx.state,
    )?;

    Ok(next_index)
}
```

Note: `span` in `Declaration::Specialize` is the body span. Verify this by checking the declaration parser. If it's the full declaration span, we need to adjust — check `syntax.rs` declaration parser to confirm what `span` covers. The body text extraction may need adjustment based on what `span` actually represents.

**Step 4: Update `ParseDispatch` blanket impl**

In `crates/kirin-chumsky/src/function_text/dispatch.rs`:

Replace imports: `HasParserEmitIR` → `ParseEmit`.

Update blanket impl bounds:
```rust
impl<L> ParseDispatch for StageInfo<L>
where
    L: Dialect + ParseEmit<L>,
    L::Type: kirin_ir::Placeholder,
    for<'t> L: HasParser<'t>,
    for<'t> L::Type: HasParser<'t, Output = L::Type>,
{
    // ... bodies unchanged
}
```

**Step 5: Update import in `parse_text.rs`**

Replace `use crate::{EmitContext, HasParser, HasParserEmitIR};` with `use crate::{HasParser, ParseEmit};`.

**Step 6: Run `cargo build -p kirin-chumsky`**

Expected: Still fails (no `ParseEmit` impls yet), but the framework side compiles in isolation.

**Step 7: Commit**

```
refactor(chumsky): update pipeline parsing to use ParseEmit
```

---

### Task 4: Update derive codegen to generate `ParseEmit` instead of witness traits

**Files:**
- Modify: `crates/kirin-derive-chumsky/src/codegen/parser/parser_emit_ir.rs` (rewrite)
- Modify: `crates/kirin-derive-chumsky/src/codegen/parser/generate.rs:36-44`
- Modify: `crates/kirin-derive-chumsky/src/codegen/bounds.rs:146-162`

**Step 1: Rewrite `parser_emit_ir.rs` to generate `ParseEmit` impl**

The file currently generates `HasParserEmitIR<'t>`. Change it to generate `ParseEmit<L>`:

For **regular (non-wrapper) enums/structs**:

```rust
quote! {
    #[automatically_derived]
    impl #impl_generics #crate_path::ParseEmit for #original_name #ty_generics
    #wc
    {
        fn parse_and_emit(
            input: &str,
            ctx: &mut #crate_path::EmitContext<'_, Self>,
        ) -> ::core::result::Result<#ir_path::Statement, ::std::vec::Vec<#crate_path::ParseError>> {
            let ast = #crate_path::parse_ast::<Self>(input)?;
            let dialect_variant = ast.0.emit_with(
                ctx,
                &|stmt, ctx| {
                    <#original_name #ty_generics as #crate_path::ParseEmit>::parse_and_emit(
                        // This recursive call re-parses from text. For nested statements
                        // (Block/Region fields), the emit_with callback receives the
                        // LanguageOutput which is itself the outer language's AST.
                        // We need to handle this differently...
                    )
                },
            )?;
            Ok(ctx.stage.statement().definition(dialect_variant).new())
        }
    }
}
```

**IMPORTANT DESIGN NOTE:** The `emit_with` helper takes an `emit_language_output` callback of type `Fn(&LanguageOutput, &mut EmitContext) -> Result<Statement, EmitError>`. This callback receives the *already parsed* `LanguageOutput` AST (not text). So within `ParseEmit::parse_and_emit`, we parse once at the top, then the recursive emit callback works with pre-parsed ASTs — it does NOT re-parse from text.

This means the `ParseEmit` impl for derives still needs the old `EmitIR`-style logic internally. The key difference is: the `for<'t>` lifetime is hidden inside the method body (introduced by `parse_ast`), not in trait bounds.

Revised codegen for regular types:

```rust
quote! {
    #[automatically_derived]
    impl #impl_generics #crate_path::ParseEmit for #original_name #ty_generics
    #wc
    {
        fn parse_and_emit(
            input: &str,
            ctx: &mut #crate_path::EmitContext<'_, Self>,
        ) -> ::core::result::Result<#ir_path::Statement, ::std::vec::Vec<#crate_path::ParseError>> {
            let ast = #crate_path::parse_ast::<Self>(input)?;
            let dialect_variant = ast.0.emit_with(
                ctx,
                &|stmt, ctx| {
                    #crate_path::EmitIR::emit(stmt, ctx)
                },
            ).map_err(|e| ::std::vec![#crate_path::ParseError {
                message: e.to_string(),
                span: ::kirin::parsers::chumsky::span::SimpleSpan::from(0..0),
            }])?;
            Ok(ctx.stage.statement().definition(dialect_variant).new())
        }
    }
}
```

Wait — this uses `EmitIR::emit(stmt, ctx)` for the language output callback, which requires `LanguageOutput: EmitIR<Self, Output = Statement>`. This bound was already present on the `EmitIR<Language>` impl for the AST. The key insight: these bounds are now on the **method body** (monomorphized at the call site), not on the **trait bound** (which would leak the lifetime).

The where clause on the `ParseEmit` impl needs:
- `for<'t> Self: HasParser<'t>` (to call `parse_ast`)
- All the value type and wrapper bounds needed for `emit_with` to work

These are the same bounds as the old `HasParserEmitIR` impl, but they're on the **`ParseEmit` impl** — NOT on any consumer's where clause. Consumers only write `L: ParseEmit<L>`.

For **wrapper structs**: delegates to the wrapped type's `ParseEmit`.

**Step 2: Update `generate.rs` to call new generator**

In `generate.rs`, replace `generate_has_parser_emit_ir_impl` call with `generate_parse_emit_impl`, and remove `generate_has_dialect_emit_ir_impl` call:

```rust
let dialect_parser_impl =
    self.generate_dialect_parser_impl(ir_input, &ast_name, crate_path);
let has_parser_impl = self.generate_has_parser_impl(ir_input, &ast_name, crate_path);
let parse_emit_impl =
    self.generate_parse_emit_impl(ir_input, &ast_name, crate_path);

quote! {
    #dialect_parser_impl
    #has_parser_impl
    #parse_emit_impl
}
```

**Step 3: Update `bounds.rs`**

Remove or rename `wrappers_emit_ir` method (line 146-162). The `HasDialectEmitIR` predicates are no longer needed.

Note: `wrappers_emit_ir` is ALSO used in `emit_ir/generate.rs` for the `emit_with` helper's where clause. Check if that usage still needs `HasDialectEmitIR` bounds or if it can switch to something else. If `emit_with` still calls `HasDialectEmitIR::emit_output` in its body (via `enum_emit.rs`), we need to update `enum_emit.rs` first (Task 5).

**Step 4: Run `cargo build -p kirin-derive-chumsky`**

Expected: May fail if `enum_emit.rs` still references `HasDialectEmitIR`. Fix in Task 5.

**Step 5: Commit**

```
refactor(derive-chumsky): generate ParseEmit instead of HasParserEmitIR
```

---

### Task 5: Remove `HasDialectEmitIR` from emit codegen

**Files:**
- Modify: `crates/kirin-derive-chumsky/src/codegen/emit_ir/enum_emit.rs:33-39`
- Modify: `crates/kirin-derive-chumsky/src/codegen/emit_ir/generate.rs:230-232`
- Modify: `crates/kirin-derive-chumsky/src/codegen/bounds.rs:146-162`

**Step 1: Update `enum_emit.rs` wrapper variant emit**

The wrapper variant emit currently calls `HasDialectEmitIR::emit_output()`. Change it to call `EmitIR::emit()` on the wrapped type's output directly, or restructure the delegation.

The `emit_with` helper method signature is:
```rust
fn emit_with<__Language, __EmitLanguageOutput>(
    &self,
    ctx: &mut EmitContext<'_, __Language>,
    emit_language_output: &__EmitLanguageOutput,
) -> Result<OriginalType, EmitError>
```

For wrapper variants, the current code delegates to `HasDialectEmitIR::emit_output`. We need to replace this with a direct call to the wrapped type's own `emit_with` method (which is generated on the wrapped type's AST).

Change `enum_emit.rs` wrapper arm from:
```rust
<#wrapped_ty as #crate_path::HasDialectEmitIR<'t, __Language, LanguageOutput>>::emit_output(
    inner, ctx, emit_language_output,
).map(Into::into)
```

To:
```rust
inner.emit_with(ctx, emit_language_output).map(Into::into)
```

This works because `inner` is the wrapped type's AST, which has its own `emit_with` method generated by derive. The `emit_with` method on the inner type handles all the emit logic.

**Step 2: Remove `wrappers_emit_ir` method from bounds**

In `bounds.rs`, remove the `wrappers_emit_ir` method entirely. Update the `emit_ir/generate.rs` helper where clause (line 230-232) to remove the `HasDialectEmitIR` bounds.

Instead, wrapper types need `emit_with` to be callable, which requires the same value type and IR type bounds — but NOT the `HasDialectEmitIR` trait bound. The bounds are already present from `wrappers_has_dialect_parser` + value type bounds.

**Step 3: Run `cargo build -p kirin-derive-chumsky`**

Expected: Compiles.

**Step 4: Commit**

```
refactor(derive-chumsky): remove HasDialectEmitIR from emit codegen
```

---

### Task 6: Remove `dialect_emit_ir.rs` codegen file

**Files:**
- Delete: `crates/kirin-derive-chumsky/src/codegen/parser/dialect_emit_ir.rs`
- Modify: `crates/kirin-derive-chumsky/src/codegen/parser/mod.rs` (remove `mod dialect_emit_ir`)

**Step 1: Remove the file and module declaration**

Delete `dialect_emit_ir.rs` and remove `mod dialect_emit_ir;` from `parser/mod.rs`.

**Step 2: Run `cargo build -p kirin-derive-chumsky`**

Expected: Compiles.

**Step 3: Commit**

```
refactor(derive-chumsky): remove HasDialectEmitIR codegen
```

---

### Task 7: Remove old witness trait definitions

**Files:**
- Delete: `crates/kirin-chumsky/src/traits/has_parser_emit_ir.rs`
- Delete: `crates/kirin-chumsky/src/traits/has_dialect_emit_ir.rs`
- Modify: `crates/kirin-chumsky/src/traits/mod.rs` (remove module declarations and re-exports)
- Modify: `crates/kirin-chumsky/src/lib.rs` (remove `HasParserEmitIR` from prelude)

**Step 1: Remove trait files**

Delete both files.

**Step 2: Update `traits/mod.rs`**

Remove:
```rust
mod has_dialect_emit_ir;
mod has_parser_emit_ir;
```
And:
```rust
pub use has_dialect_emit_ir::*;
pub use has_parser_emit_ir::*;
```

**Step 3: Update `lib.rs` prelude**

Remove `HasParserEmitIR` from the prelude. Ensure `ParseEmit` and `SimpleParseEmit` are exported.

**Step 4: Run `cargo build -p kirin-chumsky`**

Expected: Compiles if all references have been updated.

**Step 5: Commit**

```
refactor(chumsky): remove HasParserEmitIR and HasDialectEmitIR traits
```

---

### Task 8: Update snapshots and run full test suite

**Files:**
- Modify: All `.snap` files in `crates/kirin-derive-chumsky/src/codegen/parser/snapshots/`
- Modify: All `.snap` files in `crates/kirin-derive-chumsky/src/codegen/emit_ir/snapshots/`

**Step 1: Run snapshot tests and review**

```bash
cargo nextest run -p kirin-derive-chumsky
cargo insta review
```

Accept updated snapshots that reflect the `ParseEmit` codegen changes.

**Step 2: Run full workspace build and tests**

```bash
cargo build --workspace
cargo nextest run --workspace
cargo test --doc --workspace
```

Fix any remaining compilation errors from stale references.

**Step 3: Commit**

```
test(derive-chumsky): update snapshots for ParseEmit codegen
```

---

### Task 9: Verify roundtrip tests and integration tests pass

**Files:**
- Check: `tests/roundtrip/*.rs`
- Check: `example/toy-lang/`

**Step 1: Run roundtrip tests**

```bash
cargo nextest run --workspace -E 'test(roundtrip)'
```

**Step 2: Run toy-lang example**

```bash
cargo run -p kirin-toy-lang -- --help
```

**Step 3: Fix any failures**

If roundtrip tests fail, the issue is likely that test language types need `ParseEmit` impls. Since they use `#[derive(HasParser)]`, they should get it automatically from the updated derive. If manual types are used in tests, add `SimpleParseEmit` impls.

**Step 4: Commit**

```
test: verify roundtrip and integration tests with ParseEmit
```

---

### Task 10: Clean up documentation references

**Files:**
- Modify: `AGENTS.md` (if any references to `HasParserEmitIR`)
- Modify: `crates/kirin-chumsky/src/lib.rs` (module-level docs)
- Modify: `docs/plans/2026-03-09-chumsky-trait-refactor/` (update design notes)

**Step 1: Search for stale trait references**

```bash
grep -r "HasParserEmitIR\|HasDialectEmitIR" --include="*.rs" --include="*.md" .
```

Fix any remaining references.

**Step 2: Update AGENTS.md chumsky section**

Replace any mention of witness traits with `ParseEmit`.

**Step 3: Commit**

```
docs: update references from HasParserEmitIR to ParseEmit
```
