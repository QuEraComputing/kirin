# Physicist -- Ergonomics/DX Review: kirin-chumsky

## Repetition & Boilerplate

### 1. Format string annotation on every operation
Every dialect operation needs a `#[chumsky(format = "...")]` annotation. This is actually well-designed -- the format string is declarative and readable:
```rust
#[chumsky(format = "{result:name} = {.add} {lhs}, {rhs} -> {result:type}")]
```
The `.add` syntax (dot-prefixed = keyword) and `result:name`/`result:type` split are clever and low-friction once learned.

### 2. Custom type parser boilerplate
When defining a custom type lattice (like `ArithType`), the user must implement:
- `HasParser<'t>` with `parser<I>() -> BoxedParser<...>` (chumsky combinators)
- `PrettyPrint` with `namespaced_pretty_print(...)` (prettyless API)
- `DirectlyParsable` marker trait
- `Display` (for printing)
- `Placeholder` (for auto-defaults)

See `kirin-arith/src/types/arith_type.rs`: ~115 lines for what is essentially a simple enum-to-string mapping. A `#[derive(HasParser)]` for simple keyword-based type enums would eliminate most of this.

### 3. Three ParseEmit paths create confusion
The three paths (derive, SimpleParseEmit marker, manual) are documented but a new user must understand the distinction:
- `#[derive(HasParser)]` auto-generates `ParseEmit` -- preferred
- `SimpleParseEmit` marker gives a blanket impl -- for non-derive types
- Manual `ParseEmit` -- for full control

In practice, most users will use the derive. The `SimpleParseEmit` path exists for manual parser authors. The conceptual overhead of knowing three paths exist is moderate.

## Lifetime Complexity

### HasParser<'t> single lifetime is clean
The `HasParser<'t>` trait has a single lifetime for the input text. Users writing custom type parsers see:
```rust
impl<'t> HasParser<'t> for MyType {
    type Output = MyType;
    fn parser<I>() -> BoxedParser<'t, I, Self::Output>
    where I: TokenInput<'t>
    { ... }
}
```
The `'t` is present but well-scoped. The `I: TokenInput<'t>` bound is a necessary abstraction. This is acceptable complexity.

### HasDialectParser<'t> has GAT
The `HasDialectParser<'t>` trait (internal to derive) uses a GAT:
```rust
type Output<TypeOutput, LanguageOutput>: Clone + PartialEq
```
Users never implement this manually (derive generates it), so this complexity is hidden. Good design.

### EmitContext<'a, L> is straightforward
The emit context has one lifetime and one type parameter. Users encounter it only when writing manual EmitIR impls, which is rare.

## Concept Budget

### Use Case: "Add parser support for my dialect (derive path)"

| Concept | Where learned | Complexity |
|---------|--------------|------------|
| `#[derive(HasParser)]` | kirin-derive-chumsky | Low |
| `#[chumsky(format = "...")]` | attribute | Medium (format DSL syntax) |
| Format DSL: `{field}`, `{field:name}`, `{field:type}` | format docs | Medium |
| Format DSL: `{.keyword}` for namespace | format docs | Low |
| `HasParser<'t>` for custom types | kirin-chumsky | Medium |
| `DirectlyParsable` marker | kirin-chumsky | Low |
| `BoxedParser`, `TokenInput` | kirin-chumsky | Low (just follow the pattern) |
| `chumsky` combinator basics | chumsky crate | Medium-High |

**Total: ~8 concepts.** The chumsky combinator knowledge is the steepest cost, but it's only needed for custom type parsers, not for dialect operations (which use the format DSL).

### Use Case: "Parse a pipeline from text"

| Concept | Where learned | Complexity |
|---------|--------------|------------|
| `ParsePipelineText::parse` | kirin-chumsky | Low |
| `Pipeline<S>` | kirin-ir | Low |
| Stage enum with `#[derive(ParseDispatch)]` | kirin-ir | Medium |
| Text format syntax (`stage @name fn @func ...`) | implicit | Medium |

**Total: ~4 concepts.** Pleasantly minimal for file-level parsing.

### Use Case: "Parse a single statement"

| Concept | Where learned | Complexity |
|---------|--------------|------------|
| `ParseStatementTextExt::parse_statement` | kirin-chumsky | Low |
| `StageInfo<L>` or `BuilderStageInfo<L>` | kirin-ir | Low |

**Total: ~2 concepts.** Excellent ergonomics.

## Toy Scenario Results

### Scenario: Define parser for TweezerPulse dialect

With derive, this is trivially adding `#[derive(HasParser)]` and format strings:
```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, HasParser, PrettyPrint)]
#[kirin(builders, type = PulseType)]
pub enum TweezerPulse {
    #[chumsky(format = "{result:name} = {.ramp} {start}, {end}, {duration} -> {result:type}")]
    Ramp { start: SSAValue, end: SSAValue, duration: SSAValue, result: ResultValue },
    #[chumsky(format = "{result:name} = {.hold} {value}, {duration} -> {result:type}")]
    Hold { value: SSAValue, duration: SSAValue, result: ResultValue },
    __Phantom(PhantomData<PulseType>),
}
```

**What worked well:**
- Format string is readable and mirrors the desired text syntax
- Derive handles all the parser/AST/EmitIR generation
- The `.ramp`, `.hold` keywords provide automatic namespace support

**What was confusing:**
- How does the parser know what PulseType looks like? Answer: PulseType must implement `HasParser<'t>`. If you forget, the error message comes from deep in generated code.
- What does `{result:type}` print? Answer: calls `Display` on the type stored in the ResultValue. If your type doesn't impl Display, you get an error from the pretty printer, not the parser.

### Scenario: Parse a text file into a pipeline

```rust
let mut pipeline: Pipeline<Stage> = Pipeline::new();
ParsePipelineText::parse(&mut pipeline, &src)?;
```

This is excellent. Two lines. The stage enum `Stage` drives dispatch.

### Scenario: Custom type parser for PulseType

I would need ~60 lines of boilerplate:
- `impl<'t> HasParser<'t> for PulseType` with chumsky `select!` macro
- `impl Display for PulseType`
- `impl DirectlyParsable for PulseType`
- `impl PrettyPrint for PulseType`
- `impl Placeholder for PulseType`

This is the steepest cost of the whole framework for a new user. A `#[derive(HasParser)]` that works on simple keyword enums (mapping variant names to text tokens) would be transformative.

## Summary

- [P1] [confirmed] Custom type lattice requires ~60 lines of manual parser/printer/trait impls for what is conceptually a string-to-enum mapping. `kirin-arith/src/types/arith_type.rs:78-115`
- [P2] [confirmed] Error messages from missing trait impls on the type parameter (e.g., forgetting `HasParser` on PulseType) surface as deep derive-generated errors rather than clear diagnostics.
- [P3] [confirmed] Three ParseEmit paths (derive, SimpleParseEmit, manual) create conceptual overhead for new users, though in practice everyone uses derive.
- [P3] [confirmed] Format string DSL (`{.keyword}`, `{field:name}`, `{field:type}`) has no standalone documentation beyond inline examples in existing dialects.
- [P2] [likely] A `#[derive(HasParser)]` for simple keyword-type enums (like ArithType) would remove the biggest onboarding friction point for custom types.
