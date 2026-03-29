# Example 1: Pure Dialect (Arithmetic)

The simplest dialect case — no machine state, no custom errors, only base effects.

This is the pattern used by `kirin-arith`, `kirin-bitwise`, `kirin-cmp`, and `kirin-constant`.

## Key Characteristics

- `type Effect = ()` — no machine effects
- `type Error = Infallible` — no custom errors, only InterpreterError from value reads
- Returns `BaseEffect` (bind result + advance cursor) via `try_lift()`

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

    fn interpret(&self, interp: &mut I) -> Result<I::Effect<()>, I::Error<Infallible>> {
        let a = interp.read(self.lhs)?;
        let b = interp.read(self.rhs)?;
        let result = a + b;
        BaseEffect::BindValue(self.result, result).then(BaseEffect::Advance).try_lift()
    }
}
```

## Step-by-Step

1. `interp.read(self.lhs)` — reads SSA value via `ValueRead` trait. Returns `Result<I::Value, InterpreterError>`.
2. `?` propagates `InterpreterError` → converted to `I::Error<Infallible>` via `TryLift<InterpreterError>`.
3. `BaseEffect::BindValue(...).then(...)` — constructs `BaseEffect::Seq([BindValue, Advance])`.
4. `.try_lift()` — converts `BaseEffect<V>` into `Result<I::Effect<()>, ...>` via
   `TryLift<BaseEffect<V>>` on `I::Effect<()>`. The error is `Infallible` (always succeeds).

The dialect never references `SingleStageEffect` or any concrete interpreter type — it only
uses `I::Effect<()>` and `try_lift()`.
