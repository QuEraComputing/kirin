# Example 10: Error Propagation

Tracing how errors flow through the system — InterpreterError from value reads,
custom dialect errors, and Infallible from infallible lifts.

## Key Characteristics

- `type Error = ArithError` — custom error type for the dialect
- `InterpreterError` (from `read()`) lifts into `I::Error<ArithError>` via the Interpreter slot
- `ArithError` lifts into `I::Error<ArithError>` via the Machine slot
- `Infallible` (from base effect `try_lift()`) converts to any error via `From<Infallible>`

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
    type Effect = ();
    type Error = ArithError;

    fn interpret(&self, interp: &mut I) -> Result<I::Effect<()>, I::Error<ArithError>> {
        let a = interp.read(self.lhs)?;
        // InterpreterError from read() → I::Error<ArithError> via TryLift<InterpreterError>

        let b = interp.read(self.rhs)?;

        if b == I::Value::default() {
            // Custom error → I::Error<ArithError> via TryLift<ArithError> (Machine slot)
            return ArithError::DivisionByZero.try_lift();
        }

        BaseEffect::BindValue(self.result, a / b).then(BaseEffect::Advance).try_lift()
        // BaseEffect → I::Effect<()> via TryLift<BaseEffect<V>> (Base slot)
        // Infallible lift error → I::Error<ArithError> via From<Infallible>
    }
}
```

## Error Flow Summary

Three error sources, each with a different path into `I::Error<ArithError>`:

| Source | Type | Enters via | Interpreter error slot |
|---|---|---|---|
| `interp.read()` | `InterpreterError` | `?` operator | `Interpreter(err)` |
| Division by zero | `ArithError` | `.try_lift()` | `Machine(err)` |
| Base effect lift | `Infallible` | `From<Infallible>` | unreachable |

### Path 1: InterpreterError (from value read)

```
interp.read(self.lhs)        → Result<V, InterpreterError>
  ? operator                  → needs From<InterpreterError> for I::Error<ArithError>
  TryLift<InterpreterError>   → I::Error<ArithError>::Interpreter(err)
```

### Path 2: Custom error (dialect-specific)

```
ArithError::DivisionByZero   → ArithError
  .try_lift()                 → TryLift<ArithError> for I::Error<ArithError>
                              → Result<!, I::Error<ArithError>::Machine(DivisionByZero)>
```

### Path 3: Infallible (from successful base effect lift)

```
BaseEffect::BindValue(...)    → BaseEffect<V>
  .try_lift()                 → Result<I::Effect<()>, Infallible>
                              → Infallible converts to I::Error<ArithError> via From<Infallible>
                              → (never actually constructed — the lift always succeeds)
```

## Composition with Custom Errors

When composing a dialect with custom errors into a larger dialect, the error follows the
same Lift/GAT pattern as effects:

```rust
impl<I: Interpreter> Interpretable<I> for ComposedDialect<T> {
    type Error = ComposedError;

    fn interpret(&self, interp: &mut I) -> Result<I::Effect<...>, I::Error<ComposedError>> {
        match self {
            Self::CheckedDiv(op) => op.interpret(interp)?.try_lift(),
            // ArithError → ComposedError via Lift<ArithError> for ComposedError
            // I::Error<ArithError> → I::Error<ComposedError> via framework Lift between GAT instantiations
        }
    }
}
```
