# Parse Emit Simplification Design

**Date:** 2026-03-10
**Status:** Approved

## Problem

Downstream developers implementing custom parsers must deal with two derive-only
witness traits (`HasParserEmitIR`, `HasDialectEmitIR`) that exist solely to work
around E0275 recursive trait resolution. There is no documented manual
implementation path — the only way to plug into `ParseStatementText` /
`ParsePipelineText` is through `#[derive(HasParser)]` or by reverse-engineering
the witness traits.

## Solution

Replace both witness traits with a single `ParseEmit<L>` trait that internalizes
the text lifetime, eliminating GAT projection bounds entirely.

### New trait

```rust
pub trait ParseEmit<L: Dialect = Self>: Dialect {
    fn parse_and_emit(
        input: &str,
        ctx: &mut EmitContext<'_, L>,
    ) -> Result<Statement, Vec<ParseError>>;
}
```

The key insight: by combining parse + emit into one method that takes `&str`, the
intermediate AST type and its lifetime (`'t`) become internal to the method body.
No GAT projection, no `for<'t>` HRTB, no witness methods.

### Marker trait + blanket impl (simple dialects)

```rust
pub trait SimpleParseEmit: Dialect {}

impl<L> ParseEmit<L> for L
where
    L: SimpleParseEmit,
    for<'t> L: HasParser<'t>,
    for<'t> <L as HasParser<'t>>::Output: EmitIR<L, Output = L>,
{
    fn parse_and_emit(
        input: &str,
        ctx: &mut EmitContext<'_, L>,
    ) -> Result<Statement, Vec<ParseError>> {
        let ast = parse_ast::<L>(input)?;
        let variant = ast.emit(ctx).map_err(/* wrap */)?;
        Ok(ctx.stage.statement().definition(variant).new())
    }
}
```

The blanket impl only works for non-recursive dialects (no Block/Region fields)
because the `for<'t> <L as HasParser<'t>>::Output: EmitIR<L>` bound still causes
E0275 for recursive types.

### Developer paths

**Path 1: Derive (unchanged DX)**
```rust
#[derive(HasParser)]  // generates ParseEmit impl
enum MyDialect { ... }
```

**Path 2: Simple manual dialect (marker)**
```rust
impl SimpleParseEmit for MyDialect {}
// Blanket provides ParseEmit automatically
```

**Path 3: Complex manual dialect (Block/Region)**
```rust
impl ParseEmit for MyDialect {
    fn parse_and_emit(
        input: &str,
        ctx: &mut EmitContext<'_, Self>,
    ) -> Result<Statement, Vec<ParseError>> {
        let ast = parse_ast::<Self>(input)?;
        let variant = my_emit(&ast, ctx).map_err(|e| vec![...])?;
        Ok(ctx.stage.statement().definition(variant).new())
    }
}
```

## Traits removed

- `HasParserEmitIR<'t>` — replaced by `ParseEmit<L>`
- `HasDialectEmitIR<'tokens, Language, LanguageOutput>` — no longer needed

## Public API bound changes

| API | Before | After |
|-----|--------|-------|
| `ParseStatementText` impl | `for<'t> L: HasParserEmitIR<'t>` | `L: ParseEmit<L>` |
| `ParseDispatch` blanket | `for<'t> L: HasParserEmitIR<'t>` | `L: ParseEmit<L>` |
| `second_pass_concrete` | `L: HasParserEmitIR<'t>` | `L: ParseEmit<L>` |

## Derive codegen changes

- `kirin-derive-chumsky` generates `ParseEmit` instead of `HasParserEmitIR` +
  `HasDialectEmitIR`
- `HasDialectEmitIR` usage in enum emit codegen replaced by direct callback
  invocation within `ParseEmit::parse_and_emit`

## Migration

Breaking change. Blast radius limited to:

1. `kirin-chumsky` internal helpers (3-4 functions)
2. `kirin-derive-chumsky` codegen (2 files)
3. Tests with explicit `HasParserEmitIR` bounds
