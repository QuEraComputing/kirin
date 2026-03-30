# Example 10: Error Propagation

Tracing how errors flow through the system — InterpreterError from value reads,
custom dialect errors, and composed error handling.

## Key Characteristics

- `type Error = ArithError` — custom error type for the dialect
- `InterpreterError` (from `read()`) enters `InterpError` via `From` and `?`
- `ArithError` enters via `Err(InterpError::Dialect(...))`

## Code

```rust
enum ArithError {
    Overflow,
    DivisionByZero,
}

impl<I: Interpreter> Interpretable<I> for CheckedDiv<T>
where
    I::Value: Clone + Div<Output = I::Value> + PartialEq + Default,
{
    type Effect = Infallible;
    type Error = ArithError;

    fn interpret(&self, interp: &mut I)
        -> Result<Effect<I::Value, Infallible>, InterpError<ArithError>>
    {
        let a = interp.read(self.lhs)?;
        // InterpreterError from read() → InterpError<ArithError> via From + ?

        let b = interp.read(self.rhs)?;

        if b == I::Value::default() {
            // Custom error → InterpError<ArithError> via Machine variant
            return Err(InterpError::Dialect(ArithError::DivisionByZero));
        }

        Ok(Effect::BindValue(self.result, a / b).then(Effect::Advance))
    }
}
```

## Error Flow Summary

Two error sources, each with a clear path into `InterpError<ArithError>`:

| Source | Type | Enters via | InterpError variant |
|---|---|---|---|
| `interp.read()` | `InterpreterError` | `From` + `?` operator | `Interpreter(err)` |
| Division by zero | `ArithError` | `Err(InterpError::Dialect(...))` | `Dialect(err)` |

### Path 1: InterpreterError (from value read)

```
interp.read(self.lhs)        → Result<V, InterpreterError>
  ? operator                  → From<InterpreterError> for InterpError<ArithError>
                              → InterpError::Interpreter(err)
```

### Path 2: Custom error (dialect-specific)

```
ArithError::DivisionByZero   → ArithError
  InterpError::Dialect(...)   → InterpError<ArithError>::Dialect(DivisionByZero)
  Err(...)                    → Result<_, InterpError<ArithError>>
```

## Composition with Custom Errors

When composing a dialect with custom errors into a larger dialect, the error follows the
same Lift pattern as effects:

```rust
enum ComposedError {
    Arith(ArithError),
}

impl Lift<ArithError> for ComposedError {
    fn lift(from: ArithError) -> Self { Self::Arith(from) }
}

impl<I: Interpreter> Interpretable<I> for ComposedDialect<T> {
    type Error = ComposedError;

    fn interpret(&self, interp: &mut I)
        -> Result<Effect<I::Value, ...>, InterpError<ComposedError>>
    {
        match self {
            Self::CheckedDiv(op) => {
                let effect = op.interpret(interp).map_err(Lift::lift)?;
                Ok(Lift::lift(effect))
            }
        }
    }
}
```

`Lift::lift` on `InterpError<ArithError>` converts to `InterpError<ComposedError>` —
only the `Dialect(err)` variant is transformed via `Lift<ArithError> for ComposedError`.
