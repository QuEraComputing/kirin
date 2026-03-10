# Option A: Monomorphic Dispatch + Single Lifetime

> **Recommendation: This is the preferred option.**

## Problem Statement

Block/Region-containing dialect types (e.g., `FunctionBody`, `Lambda`, `If`, `For`) cannot use
`#[wraps]` in language enums because the recursive AST types overflow the trait solver when
composed under HRTB (`for<'tokens>`) bounds in `ParsePipelineText` and `ParseStatementText`.

This forces dialect authors to inline these variants manually — breaking derive composability and
requiring hand-written `Interpretable`/`SSACFGRegion` impls.

### Root Cause Chain

```
ParsePipelineText impl on Pipeline<S>
  → for<'a, 'src> S: SupportsStageDispatchMut<FirstPassAction<'a, 'src>, ...>
    → for<'src> L: HasParser<'src, 'src>
      → for<'src> <L as HasParser<'src, 'src>>::Output: EmitIR<L>
        → recursive AST types expand under placeholder lifetime
          → trait solver can't cache across placeholder lifetimes
            → E0275 overflow
```

With concrete lifetimes, Rust's trait solver detects coinductive (self-referential) cycles and
resolves them. Under HRTB (`for<'src>`), it cannot cache across placeholder lifetimes, so the
cycle overflows.

## Proposed Changes

### Change 1: Single Lifetime Collapse

**What:** Collapse `HasParser<'tokens, 'src>` to `HasParser<'t>`.

**Why:** The two-lifetime system (`'tokens` for token refs, `'src` for source string) creates
dual-bound maintenance in codegen but provides no practical benefit — the emit path already
collapses to single-lifetime `HasParser<'tokens, 'tokens>`. All AST types are consumed by
`EmitIR` and never escape the parse call, so `'src` is always equal to `'tokens` in practice.

**Before:**
```rust
pub trait HasParser<'tokens, 'src: 'tokens>: Sized + 'tokens {
    type Output: Clone + 'tokens;
    fn parser<TypeOutput, LanguageOutput>() -> impl Parser<...> + Clone;
}
```

**After:**
```rust
pub trait HasParser<'t>: Sized + 't {
    type Output: Clone + 't;
    fn parser<TypeOutput, LanguageOutput>() -> impl Parser<...> + Clone;
}
```

**Impact:**
- All `HasParser` impls (manual + derived) update from two lifetimes to one
- `BoundsBuilder` methods simplify (no more `has_parser_bounds` vs `emit_ir_bounds` split)
- Derive codegen simplifies (no more `build_ast_ty_generics_single_lifetime` special case)
- `HasDialectParser`, `HasDialectEmitIR` simplify similarly

### Change 2: Lifetime-Parameterized `ParseStatementText`

**What:** Change `ParseStatementText<L>` to `ParseStatementText<'t, L>`, eliminating the
`for<'src>` HRTB from its bounds.

**Before:**
```rust
impl<L> ParseStatementText<L> for StageInfo<L>
where
    for<'src> L: HasParser<'src, 'src>,
    for<'src> <L as HasParser<'src, 'src>>::Output: EmitIR<L, Output = Statement>,
{
    fn parse_statement(&self, input: &str) -> Result<Statement, ...> { ... }
}
```

**After:**
```rust
impl<'t, L> ParseStatementText<'t, L> for StageInfo<L>
where
    L: HasParser<'t>,
    <L as HasParser<'t>>::Output: EmitIR<L, Output = Statement>,
{
    fn parse_statement(&self, input: &'t str) -> Result<Statement, ...> { ... }
}
```

**Why:** The HRTB was needed because `parse_statement` took `&str` with an anonymous lifetime.
By parameterizing the trait with `'t`, the caller provides the concrete lifetime and no HRTB
is needed. Since `Statement` (the return type) doesn't borrow the input, this is safe.

**Impact:**
- Call sites change from `stage.parse_statement(input)` to the same (Rust infers `'t`)
- The HRTB disappears from all statement-level parsing
- `ParseStatementTextExt` helper trait updates similarly

### Change 3: Monomorphic Stage Dispatch (eliminate pipeline HRTB)

**What:** Replace the generic `SupportsStageDispatchMut` trait with a derive macro
`#[derive(ParseDispatch)]` on stage enums that generates concrete match arms.

**Before (generic dispatch):**
```rust
// Pipeline<S> requires:
for<'a, 'src> S: SupportsStageDispatchMut<FirstPassAction<'a, 'src>, ...>

// Which pushes HRTB through to every dialect type in every stage
```

**After (monomorphic dispatch):**
```rust
#[derive(StageMeta, ParseDispatch)]
pub enum Stage {
    #[stage(dialect = HighLevel)]
    High,
    #[stage(dialect = LowLevel)]
    Low,
}

// Generated code:
impl ParseDispatch for Stage {
    fn parse_statement<'t>(&self, stage: &StageInfo<Self>, input: &'t str)
        -> Result<Statement, ParseError>
    {
        match self {
            Stage::High => {
                // Concrete: HighLevel: HasParser<'t>, no HRTB
                parse_statement_concrete::<'t, HighLevel>(stage, input)
            }
            Stage::Low => {
                parse_statement_concrete::<'t, LowLevel>(stage, input)
            }
        }
    }

    fn parse_pipeline<'t>(&self, input: &'t str)
        -> Result<Pipeline, ParseError>
    {
        // Similar monomorphic dispatch for pipeline parsing
        ...
    }
}
```

**Why:** The HRTB exists because `Pipeline<S>` needs to dispatch to different dialect types
without knowing which one at compile time. By generating a match over the concrete stage
variants, each arm uses concrete `L` and `'t`, so no HRTB is needed.

**Impact:**
- `SupportsStageDispatchMut` trait is removed (or kept only as internal implementation detail)
- `Pipeline<S>` impl becomes simpler — delegates to `S::parse_pipeline()`
- Stage enum authors add `#[derive(ParseDispatch)]` alongside `#[derive(StageMeta)]`
- The derive reads the same `#[stage(dialect = L)]` annotations already present

### Change 4: Custom Parser Hooks

**What:** Add `#[chumsky(parser = expr)]` attribute for custom parser combinators at the
field or variant level.

**Example:**
```rust
#[derive(HasParser)]
#[kirin(type = MyType)]
enum MyDialect {
    #[chumsky(parser = my_custom_parser())]
    CustomOp {
        // fields parsed by the custom combinator
        result: ResultValue,
        data: Vec<SSAValue>,
    },
}
```

**Why:** Some dialect types need parser logic that can't be expressed via format strings alone
(e.g., variable-length argument lists, optional clauses, complex nesting). Currently these
require manual `HasParser` impls. A `parser` attribute lets the derive delegate to a
user-provided combinator while still generating the rest of the boilerplate (EmitIR, etc.).

**Impact:**
- New attribute parsed by `kirin-derive-chumsky`
- Derive generates: parser variant that delegates to the custom combinator, plus standard
  EmitIR/PrettyPrint for the variant
- Reduces need for fully manual parser impls

## Migration Path

1. **Change 1 (single lifetime)**: Mechanical find-and-replace across all crates. Low risk.
   All existing tests continue to work with updated signatures.

2. **Change 2 (`ParseStatementText<'t>`)**: Update trait definition + all impls + call sites.
   Low risk — Rust infers `'t` at call sites so most code is unchanged.

3. **Change 3 (`ParseDispatch` derive)**: New derive macro + update `Pipeline` impl +
   remove/simplify `SupportsStageDispatchMut`. Medium risk — the pipeline parsing logic
   is complex. Existing stage enums add one derive.

4. **Change 4 (custom parser hooks)**: Additive — no existing code changes. Can be done
   independently and incrementally.

## After the Refactor: Toy-Lang Benefits

With all HRTB eliminated, Block/Region-containing dialect types can use `#[wraps]`:

```rust
// Before (inlined):
enum HighLevel {
    FunctionBody { body: Region },      // Can't use #[wraps]
    Lambda { name: Symbol, ... },       // Can't use #[wraps]
    If { condition: SSAValue, ... },    // Can't use #[wraps]
    For { induction_var: SSAValue, ... }, // Can't use #[wraps]
    #[wraps] Arith(Arith<ArithType>),   // Works
}

// After:
enum HighLevel {
    #[wraps] FunctionBody(FunctionBody<ArithType>),  // Now works!
    #[wraps] Lambda(Lambda<ArithType>),               // Now works!
    #[wraps] If(ScfIf<ArithType>),                    // Now works!
    #[wraps] For(ScfFor<ArithType>),                  // Now works!
    #[wraps] Arith(Arith<ArithType>),
}
```

This means:
- `#[derive(Interpretable)]` works (no manual impls needed)
- `#[derive(HasParser)]` works for all variants
- Full composability — add a dialect by adding one `#[wraps]` variant

## Estimated Effort

| Change | Crates Touched | Risk | Effort |
|--------|---------------|------|--------|
| 1. Single lifetime | ~10 crates | Low | Medium |
| 2. `ParseStatementText<'t>` | 3-4 crates | Low | Small |
| 3. `ParseDispatch` derive | 4-5 crates | Medium | Large |
| 4. Custom parser hooks | 1-2 crates | Low | Medium |
| **Total** | | | **Large** |

## Pros

- **Eliminates HRTB entirely** — the root cause of E0275, not just a workaround
- **Full `#[wraps]` composability** — all dialect types work, no more inlining
- **Simpler mental model** — one lifetime, concrete dispatch, no HRTB reasoning
- **Better error messages** — concrete types produce clear errors vs HRTB overflow
- **Unified codegen** — no more dual-bound maintenance (parser vs emit paths)
- **Extensible** — custom parser hooks enable advanced use cases

## Cons

- **Large refactor** — touches ~10 crates, significant code churn
- **New derive macro** — `ParseDispatch` is an additional derive for stage authors
- **Migration burden** — all existing `HasParser` impls need updating (though mechanical)
- **Risk in pipeline rewrite** — the two-pass pipeline parsing is complex
