# Example 8: Composed Dialect (All Base Effects)

The common case — all sub-dialects use `()` as their effect type. No Lift needed.

This is the pattern for most current Kirin dialects composed into a language enum.

## Key Characteristics

- All sub-dialects have `Effect = ()` → composed effect is also `()`
- No Lift, no `try_lift()` between GAT instantiations — direct pass-through
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
    Add<T>: Interpretable<I, Effect = (), Error = Infallible>,
    Branch<T>: Interpretable<I, Effect = (), Error = Infallible>,
    Call<T>: Interpretable<I, Effect = (), Error = Infallible>,
{
    type Effect = ();
    type Error = Infallible;

    fn interpret(&self, interp: &mut I) -> Result<I::Effect<()>, I::Error<Infallible>> {
        match self {
            Self::Add(op) => op.interpret(interp),
            Self::Branch(op) => op.interpret(interp),
            Self::Call(op) => op.interpret(interp),
        }
    }
}
```

## Why No Lift Is Needed

Every sub-dialect returns `I::Effect<()>`. The composed dialect also returns `I::Effect<()>`.
The types are identical — no conversion required. This is the simplest composition case and
covers all current Kirin dialect crates (`kirin-arith`, `kirin-cf`, `kirin-scf`, `kirin-function`, etc.).
