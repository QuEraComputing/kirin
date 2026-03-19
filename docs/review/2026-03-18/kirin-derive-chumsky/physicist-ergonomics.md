# Physicist -- Ergonomics/DX Review: kirin-derive-chumsky

## Repetition & Boilerplate

### 1. Format string is the key user interface
The `#[chumsky(format = "...")]` attribute is the primary user-facing API of this crate. Its DSL is compact:
- `{field}` -- parse/print a field
- `{field:name}` -- only the name part of a ResultValue (the `%name` before `=`)
- `{field:type}` -- only the type part (after `->`)
- `{.keyword}` -- a literal keyword prefixed with the dialect namespace
- `,` and other literals -- literal tokens

This is well-designed. The format string reads like the text syntax itself. No significant repetition beyond having one format string per operation variant.

### 2. `#[derive(HasParser)]` generates 4 items silently
The derive generates: (1) an AST type, (2) a `HasDialectParser` impl, (3) `EmitIR` impls, and (4) a `ParseEmit` impl. Users don't control these individually. If something goes wrong in generated code, the error can be hard to trace back to the format string.

### 3. PrettyPrint derives share format strings with HasParser
Both `#[derive(HasParser)]` and `#[derive(PrettyPrint)]` read from `#[chumsky(format = "...")]`. This is elegant -- one format string drives both parser and printer. No duplication between the two.

## Lifetime Complexity

### Generated code uses internal lifetimes
The generated `HasDialectParser<'t>` and `HasParser<'t>` impls use `'t` but users never see these. The `ParseEmit` trait has no lifetime parameters at all. Good encapsulation.

### No HRTB in user-facing code
The old HRTB-based dispatch was replaced with `ParseDispatch`. Users write `#[derive(ParseDispatch)]` on their stage enum and never see `for<'t>` bounds. This is a significant ergonomic win.

## Concept Budget

### Use Case: "Add parser + printer for my 3-operation dialect"

| Concept | Where learned | Complexity |
|---------|--------------|------------|
| `#[derive(HasParser)]` | kirin-derive-chumsky | Low |
| `#[derive(PrettyPrint)]` | kirin-derive-chumsky | Low |
| `#[chumsky(format = "...")]` syntax | examples | Medium |
| Format field types (SSAValue, ResultValue, Block, Region, Successor, Vec) | examples | Medium |
| `HasParser<'t>` on custom types | kirin-chumsky | Medium-High |

**Total: ~5 concepts.** Very low for the parser side, assuming the type lattice already has `HasParser`.

### Use Case: "Parse a multi-dialect pipeline from text"

| Concept | Where learned | Complexity |
|---------|--------------|------------|
| `#[derive(ParseDispatch)]` on stage enum | kirin-derive-ir | Low |
| `ParsePipelineText::parse` | kirin-chumsky | Low |
| Stage text format (`stage @name ...`) | implicit/examples | Medium |
| Function text format (`fn @name(...) -> T; specialize ...`) | implicit/examples | Medium |

**Total: ~4 concepts.** The text format itself is the main learning curve.

## Toy Scenario Results

### Scenario: Write format strings for TweezerPulse operations

```rust
#[chumsky(format = "{result:name} = {.ramp} {start}, {end}, {duration} -> {result:type}")]
Ramp { start: SSAValue, end: SSAValue, duration: SSAValue, result: ResultValue },

#[chumsky(format = "{result:name} = {.hold} {value}, {duration} -> {result:type}")]
Hold { value: SSAValue, duration: SSAValue, result: ResultValue },

#[chumsky(format = "{.wait} {duration}")]
Wait { duration: SSAValue },
```

**What worked well:**
- Format strings are immediately readable as the target syntax
- `{.ramp}` auto-namespaces, so parsing `tweezer.ramp` vs just `ramp` is handled transparently
- `{result:name}` and `{result:type}` cleanly split the `%x = ... -> T` pattern

**What I had to figure out:**
- What format specifiers are available? I searched for `{.keyword}` and `{field:name}` in existing code. There is no standalone reference for the format DSL.
- How does `Vec<SSAValue>` render in a format string? Looking at `kirin-cf/src/lib.rs:32`:
  ```
  {.br} {target}({args})
  ```
  The `{args}` for `Vec<SSAValue>` is comma-separated inside parens. But the parens are in the format string, not auto-generated. This is clear once you see it.
- What about `Block` fields? Looking at `kirin-scf/src/lib.rs:57`:
  ```
  {.if} {condition} then {then_body} else {else_body}
  ```
  Block fields render as `{ ... }` blocks automatically. This is surprisingly clean.

### Scenario: Understand a compile error from a bad format string

If I write `#[chumsky(format = "{result:name} = {.ramp} {nonexistent}")]` where `nonexistent` is not a field name, the derive should produce a clear error at the attribute site. Darling-based validation likely catches this. Good.

If I write `#[chumsky(format = "{result:name} = {.ramp} {start:name}")]` where `start` is an SSAValue (not a ResultValue), applying `:name` to it should either work (SSAValues have names) or produce a clear error.

## Summary

- [P2] [confirmed] Format string DSL has no standalone documentation; users learn from examples. Creating a "Format String Reference" document would significantly improve onboarding.
- [P3] [confirmed] `#[derive(HasParser)]` generates 4 items; when errors occur in generated code, tracing back to the source format string can be difficult.
- [P3] [informational] The shared format string between HasParser and PrettyPrint is an excellent design -- single source of truth for syntax.
- [P3] [informational] `ParseDispatch` derive eliminates HRTB from user code -- significant ergonomic improvement over previous design.
