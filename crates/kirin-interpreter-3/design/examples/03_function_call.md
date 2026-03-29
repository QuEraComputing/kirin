# Example 3: Function Call (Seed Execution)

Call dialects use seeds to execute functions. The seed handles frame push, body execution,
and return value binding — the dialect just creates the seed and executes it.

This is the pattern used by `kirin-function`.

## Key Characteristics

- Still `type Effect = ()` — no machine effects (the seed handles everything)
- Uses `FunctionSeed` executed directly via `&mut I`
- Callee resolved via `PipelineAccess::resolve_callee()` on the interpreter
- No `CallEffect` type needed — seeds replace explicit Execute effects

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
    FunctionSeed<I::Value>: Execute<I>,
{
    type Effect = ();
    type Error = Infallible;

    fn interpret(&self, interp: &mut I) -> Result<I::Effect<()>, I::Error<Infallible>> {
        let args: Vec<I::Value> = self.args.iter()
            .map(|a| interp.read(*a))
            .collect::<Result<_, _>>()?;

        let callee = interp.resolve_callee(
            self.function, &args, ResolutionPolicy::UniqueLive
        )?;

        // Execute the seed directly — FunctionSeed handles frame push, run, return
        FunctionSeed {
            callee,
            args,
            results: self.results.clone(),
        }.execute(interp)?;

        BaseEffect::Advance.try_lift()
    }
}
```

## Why Seeds Instead of Effects

An earlier design had the dialect return `Execute(FunctionSeed)` as an effect. This was
dropped because:

1. **Orphan rule**: `CallEffect` (dialect crate) can't implement `Lift` for `SingleStageEffect`
   (interpreter crate) — neither crate owns both types.
2. **Unnecessary indirection**: With `&mut I` access, the dialect can execute the seed directly.
3. **Simpler API**: The dialect returns `BaseEffect::Advance` after the seed completes. No
   custom effect type to define, no Lift to implement.

The seed's `Execute<I>` impl has full `&mut I` access — it pushes frames, runs the function
body, handles the Return effect, and binds results to SSA slots.
