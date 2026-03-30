# Example 3: Function Call (Seed Execution)

Call dialects use a reusable function executor. `FunctionSeed` handles frame push, body
execution, and return value binding; the dialect just resolves the callee and delegates.

This is the pattern used by `kirin-function`.

## Key Characteristics

- Still `type Effect = Infallible` — no machine effects (the seed handles everything)
- Uses reusable `FunctionSeed` executed directly via `&mut I`
- Callee resolved via `PipelineAccess::resolve_callee()` on the interpreter
- The seed translates `Return` into a regular effect for the caller

## Code

```rust
struct Call<T> {
    function: Function,
    args: Vec<SSAValue>,
    results: Product<ResultValue>,
    _phantom: PhantomData<T>,
}

impl<I: Interpreter> Interpretable<I> for Call<T>
where
    FunctionSeed<I::Value>: Execute<I, Output = Effect<I::Value, Infallible>>,
{
    type Effect = Infallible;
    type Error = Infallible;

    fn interpret(&self, interp: &mut I)
        -> Result<Effect<I::Value, Infallible>, InterpError<Infallible>>
    {
        let args: Vec<I::Value> = self.args.iter()
            .map(|a| interp.read(*a))
            .collect::<Result<_, _>>()?;

        let callee = interp.resolve_callee(
            self.function, &args, ResolutionPolicy::UniqueLive
        )?;

        FunctionSeed {
            callee,
            args,
            results: self.results.clone(),
        }
        .execute(interp)
    }
}
```

## Why Seeds Instead of Effects

Earlier drafts encoded seed execution inside the effect algebra. That was dropped because:

1. **Wrong abstraction layer**: seeds are control programs, not observable state transitions.
2. **Unnecessary indirection**: with `&mut I` access, the dialect can execute the seed directly.
3. **Simpler effect algebra**: `Effect` stays focused on semantic state transitions, not control
   orchestration.

`FunctionSeed` earns its abstraction because it is a stable interpreter-owned entrypoint:
any call-like operation can reuse the same callee execution kernel instead of reimplementing
frame setup, body execution, and return handling. This is the same symmetry point that would
justify a dialect-defined `GraphSeed` for a reusable graph body executor.
