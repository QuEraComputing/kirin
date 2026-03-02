# Parser Infrastructure Simplification Design

## Status: Proposal

## Problem Statement

The kirin-chumsky parser infrastructure (kirin-chumsky, kirin-chumsky-derive, kirin-chumsky-format) has accumulated significant complexity across three axes:

1. **Lifetime proliferation**: Every parser trait, function, and type alias carries dual lifetimes `<'tokens, 'src>` that are almost always unified at call sites via `for<'src> L: HasParser<'src, 'src>`. This doubles the generic parameter surface of every signature.

2. **Generated type explosion**: `#[derive(HasParser)]` generates three items per dialect: `FooAST`, `FooASTSelf` (recursive wrapper), plus EmitIR impls for both. The `ASTSelf` wrapper exists solely to break the recursive type that occurs when `HasParser::Output` needs to reference itself for block/region parsing.

3. **Where clause complexity**: The EmitIR code generator (`emit_ir.rs`) builds elaborate where clauses with up to 6 categories of bounds (base, value type, wrapper From, wrapper EmitIR, wrapper HasDialectParser, dialect type parameter). These are hard to debug when trait resolution fails.

## Current Architecture

```
Source text
  -> Lexer (kirin-lexer, Logos)
  -> Token stream
  -> HasParser/HasDialectParser (chumsky combinators)
  -> AST types (FooAST<'tokens, 'src, TypeOutput, LanguageOutput>)
  -> EmitIR trait
  -> IR (Statement, Block, Region via StageInfo builders)
```

Key traits:
- `HasParser<'tokens, 'src>`: Non-recursive parser, returns `Output` (the ASTSelf type)
- `HasDialectParser<'tokens, 'src>`: Recursive parser with GAT `Output<TypeOutput, LanguageOutput>`
- `EmitIR<L: Dialect>`: Converts AST to IR, parameterized by dialect
- `DirectlyParsable`: Marker for types that parse to themselves (identity EmitIR)

The derive generates:
- `FooAST<'tokens, 'src, TypeOutput, LanguageOutput>` - The actual AST enum/struct
- `FooASTSelf<'tokens, 'src, TypeOutput>` - Newtype wrapper for recursive HasParser::Output
- `impl HasDialectParser for Foo` - Provides recursive_parser method
- `impl HasParser for Foo` - Wraps HasDialectParser with chumsky::recursive
- `impl EmitIR for FooAST` - Converts AST to IR statements
- `impl EmitIR for FooASTSelf` - Delegates to inner FooAST

## Proposed Simplifications

### 1. Collapse `'tokens` and `'src` into a single lifetime `'src`

**Rationale**: In chumsky 0.9+, the `'tokens` lifetime on `ValueInput` is structurally tied to `'src` for our use case (we always create a `Vec<Token>` from the source, then wrap it in `Stream`). Every call site already unifies them: `HasParser<'src, 'src>`, `for<'src> L: HasParser<'src, 'src>`. The second lifetime adds no expressiveness but doubles the annotation burden.

**Change**: Replace `HasParser<'tokens, 'src: 'tokens>` with `HasParser<'src>`. All type aliases (`BoxedParser`, `RecursiveParser`, `ParserError`, `TokenInput`) drop `'tokens`. The derive output and `parse_ast` function simplify accordingly.

**Risk**: Low. This is a mechanical change. The only scenario where `'tokens != 'src` would matter is if someone constructed a token buffer with a different lifetime than the source string, which the current API does not support anyway.

**Migration**: Breaking change for any downstream code that explicitly names `'tokens`. However, all known usage goes through derive macros or `parse_ast`, so manual parser implementations are rare.

### 2. Eliminate `ASTSelf` wrapper type

**Rationale**: `FooASTSelf` exists because `HasParser::Output` must be a concrete type, but the recursive parser needs `Output = AST<..., Self::Output>`, creating an infinite type. The current solution wraps the recursion in a newtype. An alternative is to make `HasParser::parser()` return a boxed parser directly without exposing the AST type in the trait's associated type.

**Change**: Remove `ASTSelf` generation entirely. Instead:
- `HasParser::Output` becomes `FooAST<'src, TypeOutput, FooAST<'src, TypeOutput, ...>>` -- but we avoid this by making `HasParser` return a boxed `Statement` directly (parse + emit in one step), or by using a type-erased `Box<dyn Any>` internally.

**Alternative approach**: Keep the two-phase architecture but use a fixed recursive wrapper. Instead of generating a per-dialect `ASTSelf`, use a single generic `RecursiveAST<T>` wrapper defined in kirin-chumsky. This halves the generated code without changing the trait design.

**Recommended approach**: Introduce `RecursiveAST<T>(T)` in kirin-chumsky. `HasParser::Output` becomes `RecursiveAST<FooAST<'src, TypeOutput, RecursiveAST<...>>>`. The derive no longer generates a per-dialect wrapper.

### 3. Simplify where clause generation with a trait alias

**Rationale**: The EmitIR impl generates 6+ categories of where bounds. Most of these follow a pattern: "for every type T that appears as a Value field and contains a type parameter, add `T: HasParser + EmitIR` bounds." This logic is scattered across `BoundsBuilder`, `collect_all_value_types_needing_bounds`, and ad-hoc generation in `generate_emit_impl`.

**Change**: Define a helper trait `ParseAndEmit<'src, L>` that bundles the common bounds:
```rust
trait ParseAndEmit<'src, L: Dialect>: HasParser<'src> where
    <Self as HasParser<'src>>::Output: EmitIR<L, Output = Self>
{}
```
Then the generated where clauses become `T: ParseAndEmit<'src, Language>` instead of two separate bounds per type. This does not reduce what the compiler checks, but halves the surface area of generated code and error messages.

### 4. Consolidate `HasParser` and `HasDialectParser`

**Rationale**: The split exists because `HasDialectParser` takes `TypeOutput` and `LanguageOutput` as method-level generics to avoid GAT projection issues, while `HasParser` provides the convenient `parser()` entry point. For dialect authors, the mental model of two traits is confusing.

**Change**: Merge into a single `HasParser<'src>` trait with two methods:
```rust
trait HasParser<'src> {
    type Output<TypeOutput, LanguageOutput>;

    fn recursive_parser<I, TypeOutput, LanguageOutput>(
        language: RecursiveParser<'src, I, LanguageOutput>,
    ) -> BoxedParser<'src, I, Self::Output<TypeOutput, LanguageOutput>>;

    fn parser<I>() -> BoxedParser<'src, I, Self::StandaloneOutput>;
}
```
This keeps the recursive entry point and the standalone entry point together. The derive generates one impl instead of two.

**Risk**: Medium. This changes the trait that dialect authors might manually implement. However, most usage is via derive, so manual impls are rare.

### 5. Reduce AST type parameters from 4 to 2

**Rationale**: `FooAST<'tokens, 'src, TypeOutput, LanguageOutput>` has 4 parameters. With lifetime collapse (proposal 1), this becomes 3. The `TypeOutput` parameter is always `<IrType as HasParser<'src>>::Output` in practice -- it exists as a separate parameter to avoid GAT projection in trait bounds. If we accept the concrete projection, we can remove `TypeOutput` from the AST type and hardcode it.

**Change**: AST types become `FooAST<'src, LanguageOutput>`. Internally, SSA/Result fields use `<L::Type as HasParser<'src>>::Output` directly. This requires the dialect's type parameter to be known at AST construction time, which it already is (it comes from the `#[kirin(type = T)]` attribute).

**Risk**: Low-medium. The AST type becomes less generic but matches actual usage.

## Prioritized Roadmap

| Priority | Change | Impact | Risk | Effort |
|----------|--------|--------|------|--------|
| P0 | Collapse dual lifetimes into `'src` | High (every signature) | Low | Medium |
| P1 | Shared `RecursiveAST<T>` wrapper | Medium (less generated code) | Low | Low |
| P1 | Consolidate HasParser + HasDialectParser | High (simpler mental model) | Medium | Medium |
| P2 | Reduce AST type params to 2 | Medium (simpler types) | Low-Med | Medium |
| P2 | `ParseAndEmit` helper trait for bounds | Medium (better errors) | Low | Low |

## Unchanged Aspects

These parts of the architecture work well and should not change:
- The `EmitIR` + `EmitContext` pattern for AST-to-IR conversion
- The format string DSL (`#[chumsky(format = "...")]`)
- The `DirectlyParsable` marker trait for identity conversions
- The `ParseStatementText` / `ParsePipelineText` API surface
- The `FieldKind` / `FieldCategory` classification system
- The validation visitor for format string correctness

## Open Questions

1. Should `HasParser` remain a standalone trait or become a supertrait pattern like the interpreter's `Interpreter<'ir>` blanket?
2. Is there appetite for a "parse directly to IR" shortcut that skips AST generation entirely, for simple value-only dialects?
3. Should the `#[chumsky(...)]` attribute namespace be unified with `#[kirin(...)]` to reduce attribute proliferation?
