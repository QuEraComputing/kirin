# Option B: Box::leak Lifetime Extension

## Approach

Keep the existing trait system largely intact but extend the input string lifetime to `'static`
using `Box::leak` before parsing, so that concrete `'static` is used instead of HRTB
`for<'src>`.

### How It Works

```rust
impl<L> ParseStatementText<L> for StageInfo<L>
where
    L: HasParser<'static, 'static>,
    <L as HasParser<'static, 'static>>::Output: EmitIR<L, Output = Statement>,
{
    fn parse_statement(&self, input: &str) -> Result<Statement, ParseError> {
        // Leak the input to get 'static lifetime
        let static_input: &'static str = Box::leak(input.to_string().into_boxed_str());

        // Parse with concrete 'static — no HRTB needed
        let result = parse_and_emit::<'static, L>(self, static_input);

        // Safety: we know Statement doesn't borrow the input
        // The leaked memory is a trade-off (small for debugging use)
        result
    }
}
```

With `'static`, the trait solver uses concrete lifetime resolution, which handles coinductive
cycles correctly — no E0275 overflow.

### Pipeline Parsing

Same approach for `ParsePipelineText`:

```rust
impl<S> ParsePipelineText<S> for Pipeline<S>
where
    S: StageMeta,
    // Concrete 'static bounds per dialect
    for_each_stage!(S, |L| {
        L: HasParser<'static, 'static>,
        <L as HasParser<'static, 'static>>::Output: EmitIR<L>,
    })
{
    fn parse(&self, input: &str) -> Result<Pipeline, ParseError> {
        let static_input: &'static str = Box::leak(input.to_string().into_boxed_str());
        // ... parse with 'static ...
    }
}
```

**Problem:** The `for_each_stage!` bound above still needs to enumerate stages. This either
requires a new trait/macro for stage enumeration, or falls back to the existing
`SupportsStageDispatchMut` pattern — which reintroduces HRTB at the pipeline level.

### Partial Fix Variant

If we only fix statement-level parsing (not pipeline), we can:
- Use `Box::leak` in `ParseStatementText` → eliminates HRTB there
- Keep `SupportsStageDispatchMut` for pipeline parsing → HRTB remains
- Block/Region types still can't use `#[wraps]` in languages used with pipeline parsing

## Impact on Toy-Lang

**Statement parsing:** Fixed — `#[wraps]` works for all types when parsing individual
statements.

**Pipeline parsing:** NOT fixed unless combined with monomorphic dispatch (which makes this
Option A with extra steps).

## Memory Implications

`Box::leak` leaks memory. For the text format (used for debugging/development, not
production parsing of large files), this is acceptable:
- Typical IR text is < 10KB
- Parse is called infrequently
- The process exits after debugging

However, if `parse_statement` is called in a loop (e.g., REPL), memory grows without bound.
A mitigation: use a bump allocator that can be reset, or `ManuallyDrop` + careful cleanup.

## Pros

- **Minimal code changes** — trait system stays the same
- **No new derive macros** — no `ParseDispatch` needed
- **Low risk** — only the parse entry points change
- **Quick to implement** — days, not weeks

## Cons

- **Memory leak** — `Box::leak` leaks the input string (acceptable for debugging, not for production)
- **Doesn't fix pipeline HRTB** — only fixes statement-level unless combined with monomorphic dispatch
- **Band-aid** — doesn't address the underlying trait system complexity
- **Two-lifetime system remains** — dual-bound maintenance continues
- **`'static` bounds are unusual** — `L: HasParser<'static, 'static>` looks surprising in trait bounds
- **No composability improvement** — doesn't enable custom parser hooks or simplify codegen
- **Partial fix only** — `#[wraps]` with Block/Region types works for statement parsing but not pipeline
