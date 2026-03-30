# Example 8: Composed Dialect (All Base Effects)

The common case — all sub-dialects use `Infallible` as their effect type. No Lift needed.

This is the pattern for most current Kirin dialects composed into a language enum.

## Key Characteristics

- All sub-dialects have `Effect = Infallible` → composed effect is also `Infallible`
- No Lift, no conversion — direct pass-through
- Derivable via `#[derive(Interpretable)]` with `#[wraps]`

## Code

```rust
#[derive(Dialect)]
enum SimpleLanguage<T> {
    Add(Add<T>),
    Branch(Branch<T>),
    Call(Call<T>),
}

impl<I: Interpreter> Interpretable<I> for SimpleLanguage<T>
where
    Add<T>: Interpretable<I, Effect = Infallible, Error = Infallible>,
    Branch<T>: Interpretable<I, Effect = Infallible, Error = Infallible>,
    Call<T>: Interpretable<I, Effect = Infallible, Error = Infallible>,
{
    type Effect = Infallible;
    type Error = Infallible;

    fn interpret(&self, interp: &mut I)
        -> Result<Effect<I::Value, Infallible>, InterpError<Infallible>>
    {
        match self {
            Self::Add(op) => op.interpret(interp),
            Self::Branch(op) => op.interpret(interp),
            Self::Call(op) => op.interpret(interp),
        }
    }
}
```

## Why No Lift Is Needed

Every sub-dialect returns `Effect<I::Value, Infallible>`. The composed dialect also returns
`Effect<I::Value, Infallible>`. The types are identical — no conversion required. This is
the simplest composition case and covers all current Kirin dialect crates (`kirin-arith`,
`kirin-cf`, `kirin-scf`, `kirin-function`, etc.).
