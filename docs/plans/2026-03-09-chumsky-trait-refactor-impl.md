# Chumsky Trait System Refactor — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task.

**Goal:** Eliminate all HRTB (`for<'src>`) bounds from the parser trait system, enabling full `#[wraps]` composability for Block/Region-containing dialect types. Refactor toy-lang to use built-in dialects exclusively.

**Architecture:** Four-phase refactor: (1) collapse two lifetimes to one across all parser traits, derive codegen, and manual impls; (2) parameterize `ParseStatementText` to eliminate statement-level HRTB; (3) introduce monomorphic `ParseDispatch` trait + derive to eliminate pipeline-level HRTB; (4) refactor toy-lang to use `#[wraps]` with built-in dialects.

**Tech Stack:** Rust proc-macro (syn, quote, darling), chumsky parser combinators, insta snapshot tests.

**No unsafe Rust.** All dispatch uses safe `HasStageInfo::try_stage_info_mut()`.

---

## Phase 1: Single Lifetime Collapse

Collapse `HasParser<'tokens, 'src: 'tokens>` → `HasParser<'t>` across the entire codebase. This is a mechanical but wide-reaching change that must happen atomically (all crates in one commit batch).

### Task 1: Core Trait Definitions

**Files:**
- Modify: `crates/kirin-chumsky/src/traits/mod.rs`
- Modify: `crates/kirin-chumsky/src/traits/has_parser.rs`
- Modify: `crates/kirin-chumsky/src/traits/has_dialect_emit_ir.rs`
- Modify: `crates/kirin-chumsky/src/traits/emit_ir.rs`
- Modify: `crates/kirin-chumsky/src/lib.rs` (prelude re-exports)

**Changes:**

1. In `traits/mod.rs`, update type aliases:
```rust
// Before
pub type ParserError<'tokens, 'src> = extra::Err<Rich<'tokens, Token<'src>, SimpleSpan>>;
pub type BoxedParser<'tokens, 'src, I, O> = Boxed<'tokens, 'tokens, I, O, ParserError<'tokens, 'src>>;
pub type RecursiveParser<'tokens, 'src, I, O> = Recursive<Direct<'tokens, 'tokens, I, O, ParserError<'tokens, 'src>>>;

// After
pub type ParserError<'t> = extra::Err<Rich<'t, Token<'t>, SimpleSpan>>;
pub type BoxedParser<'t, I, O> = Boxed<'t, 't, I, O, ParserError<'t>>;
pub type RecursiveParser<'t, I, O> = Recursive<Direct<'t, 't, I, O, ParserError<'t>>>;
```

2. In `traits/has_parser.rs`, update `TokenInput`:
```rust
// Before
pub trait TokenInput<'tokens, 'src: 'tokens>:
    chumsky::input::ValueInput<'tokens, Token = Token<'src>, Span = SimpleSpan> {}
impl<'tokens, 'src: 'tokens, I> TokenInput<'tokens, 'src> for I where
    I: chumsky::input::ValueInput<'tokens, Token = Token<'src>, Span = SimpleSpan> {}

// After
pub trait TokenInput<'t>:
    chumsky::input::ValueInput<'t, Token = Token<'t>, Span = SimpleSpan> {}
impl<'t, I> TokenInput<'t> for I where
    I: chumsky::input::ValueInput<'t, Token = Token<'t>, Span = SimpleSpan> {}
```

3. In `traits/has_parser.rs`, update `HasParser`:
```rust
// Before
pub trait HasParser<'tokens, 'src: 'tokens> {
    type Output: Clone + PartialEq;
    fn parser<I>() -> BoxedParser<'tokens, 'src, I, Self::Output>
    where I: TokenInput<'tokens, 'src>;
}

// After
pub trait HasParser<'t> {
    type Output: Clone + PartialEq;
    fn parser<I>() -> BoxedParser<'t, I, Self::Output>
    where I: TokenInput<'t>;
}
```

4. In `traits/has_parser.rs`, update `HasDialectParser`:
```rust
// Before
pub trait HasDialectParser<'tokens, 'src: 'tokens>: Sized {
    type Output<TypeOutput, LanguageOutput> where TypeOutput: 'tokens, LanguageOutput: 'tokens;
    fn recursive_parser<I, TypeOutput, LanguageOutput>(
        language: RecursiveParser<'tokens, 'src, I, LanguageOutput>,
    ) -> BoxedParser<'tokens, 'src, I, Self::Output<TypeOutput, LanguageOutput>>
    where I: TokenInput<'tokens, 'src>, TypeOutput: Clone + 'tokens, LanguageOutput: 'tokens;
    // ... namespaced_parser, clone_output, eq_output similarly
}

// After
pub trait HasDialectParser<'t>: Sized {
    type Output<TypeOutput, LanguageOutput> where TypeOutput: 't, LanguageOutput: 't;
    fn recursive_parser<I, TypeOutput, LanguageOutput>(
        language: RecursiveParser<'t, I, LanguageOutput>,
    ) -> BoxedParser<'t, I, Self::Output<TypeOutput, LanguageOutput>>
    where I: TokenInput<'t>, TypeOutput: Clone + 't, LanguageOutput: 't {
        Self::namespaced_parser::<I, TypeOutput, LanguageOutput>(language, &[])
    }
    fn namespaced_parser<I, TypeOutput, LanguageOutput>(
        language: RecursiveParser<'t, I, LanguageOutput>,
        namespace: &[&'static str],
    ) -> BoxedParser<'t, I, Self::Output<TypeOutput, LanguageOutput>>
    where I: TokenInput<'t>, TypeOutput: Clone + 't, LanguageOutput: 't;
    fn clone_output<TypeOutput, LanguageOutput>(
        output: &Self::Output<TypeOutput, LanguageOutput>,
    ) -> Self::Output<TypeOutput, LanguageOutput>
    where TypeOutput: Clone + 't, LanguageOutput: Clone + 't;
    fn eq_output<TypeOutput, LanguageOutput>(
        a: &Self::Output<TypeOutput, LanguageOutput>,
        b: &Self::Output<TypeOutput, LanguageOutput>,
    ) -> bool
    where TypeOutput: PartialEq + 't, LanguageOutput: PartialEq + 't;
}
```

Note: `recursive_parser` now has a **default implementation** delegating to `namespaced_parser`. This reduces boilerplate for manual impls from 5 required methods to 4.

5. In `traits/has_dialect_emit_ir.rs`, update supertrait:
```rust
// Before
pub trait HasDialectEmitIR<'tokens, Language: Dialect>: HasDialectParser<'tokens, 'tokens> { ... }

// After (already single lifetime, just update supertrait)
pub trait HasDialectEmitIR<'t, Language: Dialect>: HasDialectParser<'t> { ... }
```

6. Update `parse_ast` function:
```rust
// Before
pub fn parse_ast<'src, L>(input: &'src str) -> Result<L::Output, Vec<ParseError>>
where L: HasParser<'src, 'src> { ... }

// After
pub fn parse_ast<'t, L>(input: &'t str) -> Result<L::Output, Vec<ParseError>>
where L: HasParser<'t> { ... }
```

**Commit:** `refactor(chumsky): collapse HasParser to single lifetime 't`

### Task 2: Derive Codegen — AST Definition + Bounds

**Files:**
- Modify: `crates/kirin-derive-chumsky/src/codegen/ast/definition.rs`
- Modify: `crates/kirin-derive-chumsky/src/codegen/ast/trait_impls.rs`
- Modify: `crates/kirin-derive-chumsky/src/codegen/bounds.rs`
- Modify: `crates/kirin-derive-chumsky/src/codegen/mod.rs` (helper functions like `build_ast_generics`)

**Changes:**

1. Update `build_ast_generics` to produce `<'t, T..., TypeOutput, LanguageOutput>` (no `'src`).

2. In `bounds.rs`, unify `has_parser_bounds` and `emit_ir_bounds`:
```rust
// Before: two methods with different lifetime patterns
pub fn has_parser_bounds(&self, types: &[syn::Type]) -> Vec<syn::WherePredicate> {
    // T: HasParser<'tokens, 'src> + 'tokens
}
pub fn emit_ir_bounds(&self, types: &[syn::Type]) -> Vec<syn::WherePredicate> {
    // T: HasParser<'tokens, 'tokens> + 'tokens (single lifetime)
}

// After: single method
pub fn has_parser_bounds(&self, types: &[syn::Type]) -> Vec<syn::WherePredicate> {
    // T: HasParser<'t> + 't
}
pub fn emit_ir_bounds(&self, types: &[syn::Type]) -> Vec<syn::WherePredicate> {
    // T: HasParser<'t> + 't (same as has_parser_bounds now!)
    // plus <T as HasParser<'t>>::Output: EmitIR<Language, Output = T>
}
```

3. Update AST definition codegen to use single `'t` lifetime:
```rust
// Before: FooAST<'tokens, 'src, TypeOutput, LanguageOutput>
// After:  FooAST<'t, TypeOutput, LanguageOutput>
```

4. Update `PhantomData` in AST:
```rust
// Before: PhantomData<fn() -> (&'tokens (), &'src (), TypeOutput, LanguageOutput)>
// After:  PhantomData<fn() -> (&'t (), TypeOutput, LanguageOutput)>
```

5. Update Clone/PartialEq trait impl codegen in `trait_impls.rs`:
   - Wrapper variant clone: `<W as HasDialectParser<'t>>::clone_output(inner)` (was `<'tokens, 'src>`)
   - Wrapper variant eq: `<W as HasDialectParser<'t>>::eq_output(a, b)` (was `<'tokens, 'src>`)

**Commit:** `refactor(derive-chumsky): single lifetime in AST codegen and bounds`

### Task 3: Derive Codegen — Parser + Emit Impls

**Files:**
- Modify: `crates/kirin-derive-chumsky/src/codegen/parser/impl_gen.rs`
- Modify: `crates/kirin-derive-chumsky/src/codegen/parser/generate.rs`
- Modify: `crates/kirin-derive-chumsky/src/codegen/parser/dialect_emit_ir.rs`
- Modify: `crates/kirin-derive-chumsky/src/codegen/emit_ir/generate.rs`
- Modify: `crates/kirin-derive-chumsky/src/codegen/emit_ir/self_emit.rs`
- Modify: `crates/kirin-derive-chumsky/src/codegen/emit_ir/enum_emit.rs`

**Changes:**

1. In `impl_gen.rs`, update `HasParser` impl generation:
```rust
// Before: impl<'tokens, 'src: 'tokens> HasParser<'tokens, 'src> for Foo
// After:  impl<'t> HasParser<'t> for Foo
```

2. In `impl_gen.rs`, update `HasDialectParser` impl generation:
```rust
// Before: impl<'tokens, 'src: 'tokens> HasDialectParser<'tokens, 'src> for Foo
// After:  impl<'t> HasDialectParser<'t> for Foo
```

3. Remove `build_ast_ty_generics_single_lifetime` in `generate.rs` — it was a special case to collapse to single lifetime for emit. Now everything is single lifetime.

4. Remove `build_emit_impl_generics` method that strips `'src` — no `'src` to strip anymore.

5. In `dialect_emit_ir.rs`, simplify HasDialectEmitIR generation:
```rust
// Before: impl<'tokens, Language> HasDialectEmitIR<'tokens, Language> for Foo
//   where ... HasParser<'tokens, 'tokens> ...
// After:  impl<'t, Language> HasDialectEmitIR<'t, Language> for Foo
//   where ... HasParser<'t> ...
```

6. In `enum_emit.rs`, update wrapper variant emit:
```rust
// Before: <W as HasDialectEmitIR<'tokens, Language>>::emit_output(inner, ctx)
// After:  <W as HasDialectEmitIR<'t, Language>>::emit_output(inner, ctx)
```

**Commit:** `refactor(derive-chumsky): single lifetime in parser and emit codegen`

### Task 4: Update Snapshot Tests

**Files:**
- Modify: All `snapshots/*.snap` files under `crates/kirin-derive-chumsky/src/codegen/`

**Steps:**
1. Run `cargo nextest run -p kirin-derive-chumsky` — all snapshot tests will fail
2. Run `cargo insta review` — accept all updated snapshots
3. Verify snapshots show `'t` instead of `'tokens, 'src`

**Commit:** `test(derive-chumsky): update snapshots for single lifetime`

### Task 5: Manual HasParser Impls

**Files:**
- Modify: `crates/kirin-chumsky/src/builtins/primitive.rs` (bool, String)
- Modify: `crates/kirin-chumsky/src/builtins/integer.rs` (i8..usize, u8..usize)
- Modify: `crates/kirin-chumsky/src/builtins/float.rs` (f32, f64)
- Modify: `crates/kirin-arith/src/types/arith_type.rs`
- Modify: `crates/kirin-arith/src/types/arith_value.rs`
- Modify: `crates/kirin-test-types/src/simple_type.rs`
- Modify: `crates/kirin-test-types/src/unit_type.rs`
- Modify: `crates/kirin-test-types/src/value.rs`

**Changes (mechanical for all):**
```rust
// Before
impl<'tokens, 'src: 'tokens> HasParser<'tokens, 'src> for TheType {
    type Output = TheType;
    fn parser<I>() -> BoxedParser<'tokens, 'src, I, Self::Output>
    where I: TokenInput<'tokens, 'src>
    { ... }
}

// After
impl<'t> HasParser<'t> for TheType {
    type Output = TheType;
    fn parser<I>() -> BoxedParser<'t, I, Self::Output>
    where I: TokenInput<'t>
    { ... }
}
```

**Commit:** `refactor: single lifetime in all manual HasParser impls`

### Task 6: Inline Test Updates (kirin-chumsky)

**Files:**
- Modify: `crates/kirin-chumsky/src/tests.rs`
- Modify: `crates/kirin-chumsky/src/builtins/tests.rs`
- Modify: `crates/kirin-chumsky/src/function_text/tests.rs`

**Changes:**
- Update manual `HasParser` impls in test code to single lifetime
- Update any explicit `HasParser<'static, 'static>` to `HasParser<'static>`
- Update macro-generated impls in `function_text/tests.rs`

**Commit:** `test(chumsky): update inline tests for single lifetime`

### Task 7: Build and Verify Phase 1

**Steps:**
1. Run `cargo build --workspace` — fix any remaining compile errors
2. Run `cargo nextest run --workspace` — all existing tests should pass
3. Run `cargo test --doc --workspace` — doctests pass
4. Run `cargo fmt --all`

**Commit (if fixes needed):** `fix: remaining single-lifetime migration issues`

---

## Phase 2: ParseStatementText Refactor

Eliminate HRTB from statement-level parsing by making the parsing helper generic over `'t` instead of using `for<'src>`.

### Task 8: Remove HRTB from ParseStatementText

**Files:**
- Modify: `crates/kirin-chumsky/src/traits/parse_text.rs`

**Changes:**

1. Update `parse_statement_on_stage` to be generic over `'t`:
```rust
// Before
fn parse_statement_on_stage<L>(stage: &mut StageInfo<L>, input: &str) -> Result<Statement, Vec<ParseError>>
where
    L: Dialect,
    for<'src> L: HasParser<'src>,
    for<'src> <L as HasParser<'src>>::Output: EmitIR<L, Output = Statement>,

// After: The function body handles the lifetime naturally
fn parse_statement_on_stage<L>(stage: &mut StageInfo<L>, input: &str) -> Result<Statement, Vec<ParseError>>
where
    L: Dialect,
    for<'t> L: HasParser<'t>,
    for<'t> <L as HasParser<'t>>::Output: EmitIR<L, Output = Statement>,
```

Note: we still need `for<'t>` here because `input: &str` has an anonymous lifetime. The trait method `parse_statement` takes `&str`, so we can't thread the lifetime through the trait.

**Alternative approach** — keep `for<'t>` on `ParseStatementText` but simplify the bounds since single lifetime removes the `'src: 'tokens` nesting. The real HRTB fix comes from ParseDispatch in Phase 3.

The statement-level HRTB bounds `for<'t> L: HasParser<'t>` are simpler than before and shouldn't cause E0275 for non-recursive types. For Block/Region types used in pipeline parsing, Phase 3's ParseDispatch eliminates the pipeline HRTB.

**Decision:** Keep `ParseStatementText` bounds as `for<'t> L: HasParser<'t>` (simplified from `for<'src> L: HasParser<'src, 'src>`). This is a mechanical simplification. The full HRTB elimination for pipeline is Phase 3.

2. Update `StageInfo<L>` impl:
```rust
impl<L> ParseStatementText<L> for StageInfo<L>
where
    L: Dialect,
    for<'t> L: HasParser<'t>,
    for<'t> <L as HasParser<'t>>::Output: EmitIR<L, Output = Statement>,
{ ... }
```

3. Update `Pipeline<S>` impl similarly.

**Commit:** `refactor(chumsky): simplify ParseStatementText bounds to single lifetime`

### Task 9: Update Test Utilities

**Files:**
- Modify: `crates/kirin-test-utils/src/roundtrip.rs`
- Modify: `crates/kirin-test-utils/src/parser.rs`

**Changes:**
- `roundtrip.rs`: Bounds already use `StageInfo<L>: ParseStatementTextExt<L>` and `Pipeline<StageInfo<L>>: ParsePipelineText` — these are unchanged. No direct HRTB in this file.
- `parser.rs`: Update `HasParser<'src, 'src>` → `HasParser<'t>` in the parse helper.

**Commit:** `refactor(test-utils): update parser helper for single lifetime`

### Task 10: Build and Verify Phase 2

1. `cargo build --workspace`
2. `cargo nextest run --workspace`
3. `cargo fmt --all`

**Commit (if fixes):** `fix: remaining ParseStatementText migration issues`

---

## Phase 3: Monomorphic ParseDispatch

Introduce a `ParseDispatch` trait and derive macro that generates monomorphic match arms for pipeline parsing, completely eliminating HRTB from `ParsePipelineText`.

### Task 11: Design ParseDispatch Trait

**Files:**
- Create: `crates/kirin-chumsky/src/function_text/dispatch.rs`
- Modify: `crates/kirin-chumsky/src/function_text/mod.rs`

**Design:**

```rust
use kirin_ir::{CompileStage, Function, GlobalSymbol, StageMeta, StagedFunction, Statement};
use crate::ParseError;

/// Monomorphic stage dispatch for pipeline parsing.
///
/// Generated by `#[derive(ParseDispatch)]` on stage enums. Each variant
/// dispatches to the concrete dialect's parser with concrete lifetimes,
/// eliminating all HRTB bounds from the pipeline parsing path.
///
/// This replaces the generic `SupportsStageDispatchMut<FirstPassAction<'a, 'src>, ...>`
/// bounds on `Pipeline<S>`'s `ParsePipelineText` impl.
pub trait ParseDispatch: StageMeta {
    /// Execute first-pass parsing for a stage declaration or specialize header.
    ///
    /// Returns `None` if the stage_id doesn't match any variant (dialect miss).
    fn dispatch_first_pass(
        &mut self,
        stage_id: CompileStage,
        ctx: &mut FirstPassCtx<'_>,
    ) -> Result<Option<FirstPassDispatchResult>, FunctionParseError>;

    /// Execute second-pass parsing for a specialize body.
    ///
    /// Returns `None` if the stage_id doesn't match any variant (dialect miss).
    fn dispatch_second_pass(
        &mut self,
        stage_id: CompileStage,
        ctx: &mut SecondPassCtx<'_>,
    ) -> Result<Option<usize>, FunctionParseError>;
}
```

The context types bundle the action state with a single lifetime `'t` (the source text lifetime):

```rust
/// Context for first-pass dispatch.
///
/// `'t` is the lifetime of the source text being parsed. Tokens borrow
/// from the source string, and the context borrows from the token vec.
/// Using a single lifetime avoids any need for HRTB.
pub struct FirstPassCtx<'t> {
    pub tokens: &'t [(Token<'t>, SimpleSpan)],
    pub start_index: usize,
    pub function: Option<Function>,
    pub function_symbol: Option<GlobalSymbol>,
    pub staged_lookup: &'t mut FxHashMap<StagedKey, StagedFunction>,
    pub state: &'t mut ParseState,
}

pub struct SecondPassCtx<'t> {
    pub tokens: &'t [(Token<'t>, SimpleSpan)],
    pub start_index: usize,
    pub function_lookup: &'t FxHashMap<String, Function>,
    pub staged_lookup: &'t FxHashMap<StagedKey, StagedFunction>,
    pub state: &'t mut ParseState,
}
```

Inside `Pipeline::parse(src: &str)`, `'t` is the concrete lifetime of `src`. Tokens are `Vec<(Token<'_>, SimpleSpan)>` living on the stack, and the context borrows with the same lifetime. No HRTB needed — just concrete lifetime threading.

**Commit:** `feat(chumsky): add ParseDispatch trait for monomorphic stage dispatch`

### Task 12: Extract Concrete Parsing Helpers

**Files:**
- Modify: `crates/kirin-chumsky/src/function_text/parse_text.rs`

**Changes:**

Extract the parsing logic from `FirstPassAction::run` and `SecondPassSpecializeAction::run` into standalone functions that take concrete `L` and a concrete lifetime `'t`:

```rust
/// First-pass parsing for a concrete dialect L with a concrete lifetime.
pub fn first_pass_concrete<'t, L>(
    stage: &mut StageInfo<L>,
    stage_id: CompileStage,
    ctx: &mut FirstPassCtx<'t>,
) -> Result<FirstPassDispatchResult, FunctionParseError>
where
    L: Dialect + HasParser<'t>,
    L::Type: kirin_ir::Placeholder + HasParser<'t, Output = L::Type>,
    <L as HasParser<'t>>::Output: EmitIR<L, Output = Statement>,
{
    // Move parsing logic from FirstPassAction::run here
    let (declaration, consumed_span) =
        parse_one_declaration::<L>(&ctx.tokens[ctx.start_index..])
            .map_err(parse_error_from_chumsky)?;
    // ... rest of logic
}

pub fn second_pass_concrete<'t, L>(
    stage: &mut StageInfo<L>,
    stage_id: CompileStage,
    ctx: &mut SecondPassCtx<'t>,
) -> Result<usize, FunctionParseError>
where
    L: Dialect + HasParser<'t>,
    L::Type: HasParser<'t, Output = L::Type>,
    <L as HasParser<'t>>::Output: EmitIR<L, Output = Statement>,
{
    // Move parsing logic from SecondPassSpecializeAction::run here
}
```

**Critical insight:** These helpers use `'t` as a **generic lifetime parameter**, NOT `for<'t>` HRTB. The caller provides a concrete `'t` from the pipeline's `parse()` method (the lifetime of the source string). Since `L` is also concrete in the generated `ParseDispatch` match arms, there is **zero HRTB** in the entire dispatch chain. The trait solver resolves `HighLevel: HasParser<'t>` directly from the derive-generated impl without any recursive expansion.

Make `FirstPassCtx`, `SecondPassCtx`, `FirstPassDispatchResult`, `ParseState`, `StagedKey`, `DeclKeyword`, `FirstPassOutcome` all `pub(crate)` so the derive-generated code (which lives in a different crate but generates code that compiles in the user's crate) can reference them, OR re-export them as needed.

**Important design note:** The generated `ParseDispatch` impl will be in the user's crate (generated by the derive macro). It needs access to these helper functions. Export them from `kirin_chumsky`:

```rust
// In kirin-chumsky/src/function_text/mod.rs
pub use parse_text::{
    FirstPassCtx, SecondPassCtx, FirstPassDispatchResult,
    first_pass_concrete, second_pass_concrete,
};
```

**Commit:** `refactor(chumsky): extract concrete parsing helpers for ParseDispatch`

### Task 13: Implement ParseDispatch Derive Macro

**Files:**
- Modify: `crates/kirin-derive-toolkit/src/stage.rs` (reuse stage attribute parsing)
- Create: `crates/kirin-derive-toolkit/src/parse_dispatch.rs` (code generation)
- Modify: `crates/kirin-derive-ir/src/lib.rs` (register proc-macro entry point)

**The derive reads the same `#[stage(dialect = L)]` attributes as StageMeta and generates:**

```rust
// Input:
#[derive(StageMeta, ParseDispatch)]
enum MyStage {
    #[stage(name = "source", dialect = HighLevel)]
    Source(StageInfo<HighLevel>),
    #[stage(name = "target", dialect = LowLevel)]
    Target(StageInfo<LowLevel>),
}

// Generated (note: 't is concrete from the caller, no HRTB):
impl kirin_chumsky::ParseDispatch for MyStage {
    fn dispatch_first_pass(
        &mut self,
        stage_id: CompileStage,
        ctx: &mut kirin_chumsky::FirstPassCtx<'_>,
    ) -> Result<Option<kirin_chumsky::FirstPassDispatchResult>, kirin_chumsky::FunctionParseError> {
        // Each arm has concrete L and concrete 't (from ctx). No HRTB.
        if let Some(stage_info) = <Self as HasStageInfo<HighLevel>>::try_stage_info_mut(self) {
            return kirin_chumsky::first_pass_concrete::<HighLevel>(stage_info, stage_id, ctx)
                .map(Some);
        }
        if let Some(stage_info) = <Self as HasStageInfo<LowLevel>>::try_stage_info_mut(self) {
            return kirin_chumsky::first_pass_concrete::<LowLevel>(stage_info, stage_id, ctx)
                .map(Some);
        }
        Ok(None)
    }

    fn dispatch_second_pass(
        &mut self,
        stage_id: CompileStage,
        ctx: &mut kirin_chumsky::SecondPassCtx<'_>,
    ) -> Result<Option<usize>, kirin_chumsky::FunctionParseError> {
        if let Some(stage_info) = <Self as HasStageInfo<HighLevel>>::try_stage_info_mut(self) {
            return kirin_chumsky::second_pass_concrete::<HighLevel>(stage_info, stage_id, ctx)
                .map(Some);
        }
        if let Some(stage_info) = <Self as HasStageInfo<LowLevel>>::try_stage_info_mut(self) {
            return kirin_chumsky::second_pass_concrete::<LowLevel>(stage_info, stage_id, ctx)
                .map(Some);
        }
        Ok(None)
    }
}
```

**Key implementation notes:**
- Reuse `extract_dialect_type()` from `stage.rs` to get each variant's dialect type
- Each match arm calls `HasStageInfo::<L>::try_stage_info_mut(self)` (safe, no unsafe)
- Each arm delegates to `first_pass_concrete::<L>()` / `second_pass_concrete::<L>()`
- The `for<'t> L: HasParser<'t>` bounds are satisfied by the concrete dialect types

**Crate path:** The derive needs to reference `kirin_chumsky` types. Add a `#[stage(chumsky_crate = kirin_chumsky)]` attribute (or use a reasonable default like `::kirin::parsers`).

**Commit:** `feat(derive-ir): add ParseDispatch derive macro`

### Task 14: Update Pipeline to Use ParseDispatch

**Files:**
- Modify: `crates/kirin-chumsky/src/function_text/parse_text.rs`

**Changes:**

Replace the HRTB-based `ParsePipelineText` impl with one that uses `ParseDispatch`:

```rust
// Before
impl<S> ParsePipelineText for Pipeline<S>
where
    S: StageMeta,
    for<'a, 'src> S: SupportsStageDispatchMut<
        FirstPassAction<'a, 'src>, FirstPassDispatchResult, FunctionParseError>,
    for<'a, 'src> S: SupportsStageDispatchMut<
        SecondPassSpecializeAction<'a, 'src>, usize, FunctionParseError>,
{ ... }

// After
impl<S> ParsePipelineText for Pipeline<S>
where
    S: StageMeta + ParseDispatch,
{
    fn parse(&mut self, src: &str) -> Result<Vec<Function>, FunctionParseError> {
        let tokens = tokenize(src);
        // ... same two-pass logic but using:

        // Pass 1: dispatch via ParseDispatch
        let mut first_ctx = FirstPassCtx {
            tokens: &tokens,
            start_index: index,
            function,
            function_symbol,
            staged_lookup: &mut staged_lookup,
            state: &mut state,
        };
        let result = self.stage_mut(stage_id)
            .ok_or_else(|| ...)?
            .dispatch_first_pass(stage_id, &mut first_ctx)?
            .ok_or_else(|| ...)?;

        // Pass 2: similar with SecondPassCtx
    }
}
```

The `SupportsStageDispatchMut<FirstPassAction<'a, 'src>, ...>` bounds are entirely replaced by `S: ParseDispatch`. No HRTB anywhere.

**Keep the old StageActionMut impls** for FirstPassAction and SecondPassSpecializeAction temporarily (they're still used as the concrete logic inside `first_pass_concrete`/`second_pass_concrete`), or inline that logic directly. Decide based on what's cleaner.

**Commit:** `refactor(chumsky): Pipeline uses ParseDispatch, eliminating HRTB`

### Task 15: Build and Verify Phase 3

**Files:**
- Modify: `crates/kirin-chumsky/src/function_text/tests.rs` (update test stage enums to derive ParseDispatch)

1. Update test stage types in `function_text/tests.rs` to use `#[derive(ParseDispatch)]`
2. `cargo build --workspace`
3. `cargo nextest run --workspace`
4. `cargo fmt --all`

**Commit:** `test(chumsky): update pipeline tests for ParseDispatch`

---

## Phase 4: Test & Example Updates

### Task 16: Update Test Languages

**Files:**
- Modify: `crates/kirin-test-languages/src/arith_function_language.rs`
- Modify: `crates/kirin-test-languages/src/bitwise_function_language.rs`
- Modify: `crates/kirin-test-languages/src/callable_language.rs`
- Modify: `crates/kirin-test-languages/src/simple_language.rs`
- Modify: `crates/kirin-test-languages/src/composite_language.rs`
- Modify: `crates/kirin-test-languages/src/namespaced_language.rs`

**Changes:**
- Any test languages that have associated stage types need `#[derive(ParseDispatch)]`
- Most test languages use `Pipeline<StageInfo<L>>` (single dialect), which doesn't need ParseDispatch since `StageInfo<L>` is not a stage enum

**Commit:** `refactor(test-languages): add ParseDispatch where needed`

### Task 17: Update Integration Tests

**Files:**
- Modify: `tests/roundtrip/arith.rs`
- Modify: `tests/roundtrip/bitwise.rs`
- Modify: `tests/roundtrip/cf.rs`
- Modify: `tests/roundtrip/cmp.rs`
- Modify: `tests/roundtrip/constant.rs`
- Modify: `tests/roundtrip/function.rs`
- Modify: `tests/roundtrip/scf.rs`
- Modify: `tests/roundtrip/composite.rs`
- Modify: `tests/roundtrip/namespace.rs`
- Modify: `example/simple.rs`

**Changes:**
- Roundtrip tests that define inline languages/stages: add `#[derive(ParseDispatch)]` where needed
- Tests using `Pipeline<StageInfo<L>>`: these should still work since `StageInfo<L>` can implement `ParseDispatch` with a trivial single-dialect dispatch (or we provide a blanket impl for `StageInfo<L>`):

```rust
// Blanket impl for single-dialect pipelines.
// The `for<'t>` bounds here are on concrete L (provided by the caller),
// so the trait solver resolves them without overflow. This is NOT the
// problematic HRTB pattern — L is fixed, only 't varies.
impl<L> ParseDispatch for StageInfo<L>
where
    L: Dialect,
    L::Type: Placeholder,
    for<'t> L: HasParser<'t>,
    for<'t> L::Type: HasParser<'t, Output = L::Type>,
    for<'t> <L as HasParser<'t>>::Output: EmitIR<L, Output = Statement>,
{
    fn dispatch_first_pass(&mut self, stage_id: CompileStage, ctx: &mut FirstPassCtx<'_>)
        -> Result<Option<FirstPassDispatchResult>, FunctionParseError>
    {
        first_pass_concrete::<L>(self, stage_id, ctx).map(Some)
    }
    // ... second_pass similarly
}
```

This blanket impl means `Pipeline<StageInfo<L>>` still works for single-dialect pipelines without any changes to existing tests. The `for<'t> L: HasParser<'t>` bounds are safe because `L` is concrete — the trait solver only needs to prove `ConcreteType: HasParser<'t>` for a universally-quantified `'t`, which is trivially satisfied by the derive-generated `impl<'t> HasParser<'t> for ConcreteType`. The problematic HRTB pattern was `for<'t> L: HasParser<'t>` where BOTH `L` AND `'t` were unknown during trait resolution in the recursive tuple dispatch.

**Commit:** `test: update integration tests for ParseDispatch`

### Task 18: Full Test Suite Verification

1. `cargo build --workspace`
2. `cargo nextest run --workspace`
3. `cargo test --doc --workspace`
4. `cargo fmt --all`

**Commit (if needed):** `fix: remaining integration issues`

---

## Phase 5: Toy-Lang Refactor

Switch toy-lang from inlined dialect variants to `#[wraps]` with built-in dialect types.

### Task 19: Refactor Toy-Lang Language Definitions

**Files:**
- Modify: `example/toy-lang/src/language.rs`

**Changes:**

The key change: inlined variants (FunctionBody, Lambda, If, For, Yield, Constant) become `#[wraps]` variants using built-in dialect types.

For `HighLevel`:
```rust
// Before (inlined):
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, HasParser, PrettyPrint)]
#[kirin(fn, type = ArithType)]
pub enum HighLevel {
    #[kirin(constant, pure)]
    #[chumsky(format = "{result:name} = {.constant} {value} -> {result:type}")]
    Constant { value: ArithValue, result: ResultValue },
    #[chumsky(format = "{body}")]
    FunctionBody { body: Region },
    #[chumsky(format = "{res:name} = {.lambda} ...")]
    Lambda { name: Symbol, captures: Vec<SSAValue>, body: Region, res: ResultValue },
    #[chumsky(format = "{.if} {condition} then {then_body} else {else_body}")]
    If { condition: SSAValue, then_body: Block, else_body: Block },
    #[chumsky(format = "{.for} ...")]
    For { induction_var: SSAValue, start: SSAValue, end: SSAValue, step: SSAValue, body: Block },
    #[kirin(terminator)]
    #[chumsky(format = "{.yield} {value}")]
    Yield { value: SSAValue },
    #[wraps] Arith(Arith<ArithType>),
    #[wraps] Cmp(Cmp<ArithType>),
    #[wraps] Bitwise(Bitwise<ArithType>),
    #[wraps] Call(Call<ArithType>),
    #[wraps] Return(Return<ArithType>),
}

// After (all #[wraps]):
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, HasParser, PrettyPrint)]
#[kirin(fn, type = ArithType)]
pub enum HighLevel {
    #[wraps] Constant(kirin_constant::Constant<ArithType>),
    #[wraps] FunctionBody(kirin_function::FunctionBody<ArithType>),
    // Lambda, If, For, Yield — need to check if built-in types exist in kirin-scf/kirin-function
    // If not, keep inlined or create them
    #[wraps] Arith(Arith<ArithType>),
    #[wraps] Cmp(Cmp<ArithType>),
    #[wraps] Bitwise(Bitwise<ArithType>),
    #[wraps] Call(Call<ArithType>),
    #[wraps] Return(Return<ArithType>),
}
```

**Built-in dialect type mapping (all verified to exist):**

| Toy-lang inline variant | Built-in type | Block/Region? |
|---|---|---|
| `Constant { value, result }` | `kirin_constant::Constant<ArithValue, ArithType>` | No |
| `FunctionBody { body }` | `kirin_function::FunctionBody<ArithType>` | Region |
| `Lambda { name, captures, body, res }` | `kirin_function::Lambda<ArithType>` | Region |
| `If { condition, then_body, else_body }` | `kirin_scf::If<ArithType>` | Block x2 |
| `For { induction_var, start, end, step, body }` | `kirin_scf::For<ArithType>` | Block |
| `Yield { value }` | `kirin_scf::Yield<ArithType>` | No |

All inlined variants have exact built-in equivalents. The full `HighLevel` enum becomes:
```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, HasParser, PrettyPrint, Interpretable)]
#[kirin(fn, type = ArithType)]
pub enum HighLevel {
    #[wraps] Constant(kirin_constant::Constant<ArithValue, ArithType>),
    #[wraps] FunctionBody(kirin_function::FunctionBody<ArithType>),
    #[wraps] Lambda(kirin_function::Lambda<ArithType>),
    #[wraps] If(kirin_scf::If<ArithType>),
    #[wraps] For(kirin_scf::For<ArithType>),
    #[wraps] Yield(kirin_scf::Yield<ArithType>),
    #[wraps] Arith(Arith<ArithType>),
    #[wraps] Cmp(Cmp<ArithType>),
    #[wraps] Bitwise(Bitwise<ArithType>),
    #[wraps] Call(Call<ArithType>),
    #[wraps] Return(Return<ArithType>),
}
```

Similarly for `LowLevel`:
```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, HasParser, PrettyPrint, Interpretable)]
#[kirin(fn, type = ArithType)]
pub enum LowLevel {
    #[wraps] Constant(kirin_constant::Constant<ArithValue, ArithType>),
    #[wraps] FunctionBody(kirin_function::FunctionBody<ArithType>),
    #[wraps] Arith(Arith<ArithType>),
    #[wraps] Cmp(Cmp<ArithType>),
    #[wraps] Bitwise(Bitwise<ArithType>),
    #[wraps] Cf(ControlFlow<ArithType>),
    #[wraps] Bind(Bind<ArithType>),
    #[wraps] Call(Call<ArithType>),
    #[wraps] Return(Return<ArithType>),
}
```

**Important:** With HRTB eliminated, inlined variants WITH Block/Region fields now work with `#[derive(HasParser)]`. So even if some types must stay inlined, the derive still works. The main benefit of `#[wraps]` is that `#[derive(Interpretable)]` also works automatically.

**Commit:** `refactor(toy-lang): use #[wraps] with built-in dialect types`

### Task 20: Remove Manual Interpreter Impls

**Files:**
- Modify: `example/toy-lang/src/interpret.rs`

**Changes:**

If all variants use `#[wraps]`, the manual `Interpretable` and `SSACFGRegion` impls can be replaced with `#[derive(Interpretable)]` on the language enum. The derive auto-delegates to each wrapped type's interpreter impl.

```rust
// Before: ~200 lines of manual match arms
impl<'ir, I> Interpretable<'ir, I, HighLevel> for HighLevel
where ... { fn interpret(&self, interp: &mut I) -> ... { match self { ... } } }

// After: derive handles everything
#[derive(Interpretable)]
pub enum HighLevel { ... }
```

Remove the `SSACFGRegion` manual impls too — the derive generates delegation for `#[wraps]` variants that implement it.

**Commit:** `refactor(toy-lang): replace manual Interpretable impls with derive`

### Task 21: Add ParseDispatch to Toy-Lang Stage

**Files:**
- Modify: `example/toy-lang/src/language.rs` (stage enum)
- Modify: `example/toy-lang/src/main.rs` (pipeline parsing)

**Changes:**

Add `#[derive(ParseDispatch)]` to the toy-lang stage enum:
```rust
#[derive(Debug, Clone, StageMeta, RenderStage, ParseDispatch)]
pub enum ToyStage {
    #[stage(name = "source", dialect = HighLevel)]
    Source(StageInfo<HighLevel>),
    #[stage(name = "target", dialect = LowLevel)]
    Target(StageInfo<LowLevel>),
}
```

Update `main.rs` to use `Pipeline<ToyStage>` (if not already).

**Commit:** `feat(toy-lang): add ParseDispatch derive to stage enum`

### Task 22: Verify Toy-Lang E2E Tests

1. `cargo nextest run -p toy-lang`
2. Run the toy-lang binary manually with example inputs
3. Verify all 10 e2e tests pass

**Commit (if fixes):** `fix(toy-lang): resolve remaining e2e test issues`

---

## Phase 6: Cleanup and Documentation

### Task 23: Cleanup Old Dispatch Code

**Files:**
- Possibly modify: `crates/kirin-chumsky/src/function_text/parse_text.rs`

**Changes:**
- Remove `FirstPassAction` and `SecondPassSpecializeAction` structs (now replaced by context types)
- Remove their `StageActionMut` impls (logic moved to `first_pass_concrete`/`second_pass_concrete`)
- OR keep them if `first_pass_concrete` delegates to them internally

The `SupportsStageDispatchMut` trait itself stays — it's still used by the interpreter for runtime dispatch. Only the parser-specific usage is replaced by ParseDispatch.

**Commit:** `refactor(chumsky): remove parser-specific StageActionMut impls`

### Task 24: Update MEMORY.md and CLAUDE.md

**Files:**
- Modify: `/Users/roger/.claude/projects/-Users-roger-Code-rust-kirin/memory/MEMORY.md`
- Possibly modify: `AGENTS.md`

**Changes:**
- Update "Chumsky Parser Conventions" section to reflect single lifetime
- Update "Two-Crate-Versions Problem" if relevant
- Add note about ParseDispatch derive
- Remove references to two-lifetime system

### Task 25: Final Verification

1. `cargo build --workspace`
2. `cargo nextest run --workspace`
3. `cargo test --doc --workspace`
4. `cargo fmt --all`
5. Review all changes for no unsafe code
6. Verify toy-lang e2e tests all pass

---

## Task Dependency Graph

```
Phase 1: Single Lifetime
  Task 1 (core traits) → Task 2 (derive AST/bounds) → Task 3 (derive parser/emit)
  Task 3 → Task 4 (snapshots)
  Task 1 → Task 5 (manual impls)
  Task 5 → Task 6 (inline tests)
  All → Task 7 (verify)

Phase 2: ParseStatementText
  Task 7 → Task 8 (remove HRTB)
  Task 8 → Task 9 (test utils)
  Task 9 → Task 10 (verify)

Phase 3: ParseDispatch
  Task 10 → Task 11 (trait design)
  Task 11 → Task 12 (concrete helpers)
  Task 12 → Task 13 (derive macro)
  Task 13 → Task 14 (Pipeline update)
  Task 14 → Task 15 (verify)

Phase 4: Test Updates
  Task 15 → Task 16 (test languages)
  Task 16 → Task 17 (integration tests)
  Task 17 → Task 18 (verify)

Phase 5: Toy-Lang
  Task 18 → Task 19 (language defs)
  Task 19 → Task 20 (remove manual impls)
  Task 20 → Task 21 (ParseDispatch)
  Task 21 → Task 22 (verify)

Phase 6: Cleanup
  Task 22 → Task 23 (cleanup)
  Task 23 → Task 24 (docs)
  Task 24 → Task 25 (final verify)
```

## Ergonomic Design Notes

### For derive-based parsers (most common):
- `#[derive(Dialect, HasParser, PrettyPrint)]` generates everything
- `#[derive(ParseDispatch)]` alongside `#[derive(StageMeta)]` for stage enums
- No lifetime annotations needed in user code
- `#[wraps]` works for ALL dialect types including Block/Region-containing ones

### For manual HasParser implementors:
- Single lifetime `impl<'t> HasParser<'t> for MyType` (simpler than two lifetimes)
- `recursive_parser` has a default impl (delegates to `namespaced_parser`)
- 4 required items: `Output` type, `namespaced_parser`, `clone_output`, `eq_output`
- Witness methods follow a simple pattern: `output.clone()` and `a == b` for leaf types

### For manual HasDialectParser implementors:
- Same 4-item pattern as above
- `clone_output` and `eq_output` are boilerplate but necessary for GAT composition safety
- Consider providing a helper in docs showing the standard pattern

### Witness method rationale (kept, not removed):
- `clone_output`/`eq_output` solve a fundamentally different problem (GAT projection bounds for Clone/PartialEq on self-referential AST types) than the HRTB issue
- They cannot be eliminated without either unsafe code or losing type safety
- The single-lifetime refactor simplifies them (one fewer lifetime param) but doesn't remove the need
- `emit_output` on `HasDialectEmitIR` remains a separate trait for dialect-specific bound carriage
