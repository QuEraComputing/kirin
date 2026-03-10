# Option C: Type-Erased Emit (Box<dyn EmitIR>)

## Approach

Break the HRTB chain by type-erasing the `EmitIR` bound on parser output. Instead of
requiring `<L as HasParser<'src, 'src>>::Output: EmitIR<L>` (which must hold for all `'src`),
the parser returns a `Box<dyn Emit>` that carries the emit logic as a vtable.

### Core Idea

```rust
/// Type-erased emit trait (no generics, no lifetime params on trait)
pub trait DynEmit {
    fn emit(&self, ctx: &mut EmitContextDyn) -> Result<Statement, EmitError>;
}

/// Parser output wraps a dyn-emit
pub struct ParsedAST<'t> {
    inner: Box<dyn DynEmit + 't>,
}

impl<'t> ParsedAST<'t> {
    pub fn emit<L: Dialect>(&self, ctx: &mut EmitContext<'_, L>) -> Result<Statement, EmitError> {
        // Downcast or use erased context
        self.inner.emit(&mut ctx.erase())
    }
}
```

### Parser Trait Change

```rust
pub trait HasParser<'t>: Sized + 't {
    // Output is always ParsedAST — no EmitIR bound needed
    fn parser<TypeOutput, LanguageOutput>() -> impl Parser<'t, ..., ParsedAST<'t>> + Clone;
}
```

Since `ParsedAST` carries the emit logic inside itself (via vtable), there's no need for
`<Output as EmitIR<L>>` bounds anywhere. The HRTB chain is broken at the parser output.

### EmitContext Erasure

The challenge: `EmitContext<'_, L>` is generic over `L: Dialect`. To call through `dyn DynEmit`,
we need a type-erased context:

```rust
pub struct EmitContextDyn<'a> {
    // Builder methods that work without knowing L
    region_builder: &'a mut dyn RegionBuilder,
    block_builder: &'a mut dyn BlockBuilder,
    type_table: &'a mut dyn TypeTable,
    // ...
}
```

This requires abstracting all `EmitContext` operations behind trait objects, which is a
significant architectural change.

### Where Dialect-Specific Logic Goes

The `L: Dialect` parameter is currently used in `EmitIR` for:
1. `L::Type` — the type system (used in `ResultValue` allocation)
2. `From<DialectType> for L` — converting dialect ops to language statements
3. Stage info — for block/region construction

With type erasure, these become:
1. Types are erased to `Box<dyn Any>` or a trait object
2. Conversion uses a registered closure/vtable
3. Stage info passed through erased context

## Impact on Toy-Lang

Would fix `#[wraps]` composability for all dialect types since the HRTB chain is fully broken.

## Pros

- **Fully breaks HRTB chain** — no `EmitIR` bound needed on parser output
- **Clean separation** — parser and emit are independently composable
- **No monomorphic dispatch needed** — generic dispatch works because bounds are simpler
- **No new derive macros** — existing derives adapt

## Cons

- **Massive architectural change** — requires type-erasing `EmitContext`, `Dialect::Type`, etc.
- **Runtime overhead** — vtable dispatch on every emit operation, `Box` allocation per parsed node
- **Loss of type safety** — `Box<dyn Any>` for types, runtime errors instead of compile-time
- **Very high risk** — changes the fundamental architecture of the parser-emit pipeline
- **Hardest to implement** — estimated weeks of work, touching every crate
- **Custom parser hooks harder** — custom combinators need to produce `ParsedAST`, adding boilerplate
- **Debugging harder** — type-erased errors are less informative
- **Performance regression** — allocation per AST node, virtual dispatch in emit path
