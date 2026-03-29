# Example 1: Pure Dialect (Arithmetic)

The simplest dialect case — no machine state, no custom errors, only base effects.

This is the pattern used by `kirin-arith`, `kirin-bitwise`, `kirin-cmp`, and `kirin-constant`.

## Key Characteristics

- `type Effect = ()` — no machine effects
- `type Error = Infallible` — no custom errors, only InterpreterError from value reads
- Returns `Effect` variants directly (BindValue + Advance)

## Code

```rust
struct Add<T> {
    lhs: SSAValue,
    rhs: SSAValue,
    result: ResultValue,
    _phantom: PhantomData<T>,
}

impl<I: Interpreter> Interpretable<I> for Add<T>
where
    I::Value: Clone + std::ops::Add<Output = I::Value>,
{
    type Effect = ();
    type Error = Infallible;

    fn interpret(&self, interp: &mut I)
        -> Result<Effect<I::Value, I::Seed, ()>, InterpError<Infallible>>
    {
        let a = interp.read(self.lhs)?;
        let b = interp.read(self.rhs)?;
        let result = a + b;
        Ok(Effect::BindValue(self.result, result).then(Effect::Advance))
    }
}
```

## Step-by-Step

1. `interp.read(self.lhs)` — reads SSA value via `ValueRead` trait. Returns `Result<I::Value, InterpreterError>`.
2. `?` propagates `InterpreterError` → converted to `InterpError<Infallible>` via `From<InterpreterError>`.
3. `Effect::BindValue(...).then(Effect::Advance)` — constructs `Effect::Seq([BindValue, Advance])`.
4. `Ok(...)` — wraps the effect in the Ok position of the result.

The dialect constructs `Effect` variants directly — no `try_lift()`, no GAT, no indirection.
