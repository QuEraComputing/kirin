# Error Model

Errors follow the same Lift pattern as effects. A single `InterpError<ME>` type wraps
base interpreter errors and dialect machine errors.

## Base Interpreter Errors

Shared error vocabulary for all interpreters:

```rust
enum InterpreterError {
    NoFrame,
    UnboundValue(SSAValue),
    FuelExhausted,
    ArityMismatch { expected: usize, got: usize },
    StageResolution { stage: CompileStage, kind: StageResolutionError },
    // ...
}
```

## Unified Error Type

```rust
enum InterpError<ME> {
    Interpreter(InterpreterError),
    Machine(ME),
}
```

`From<InterpreterError>` enables the `?` operator on `read()` calls:

```rust
impl<ME> From<InterpreterError> for InterpError<ME> {
    fn from(e: InterpreterError) -> Self { Self::Interpreter(e) }
}
```

## Dialect Error Usage

**Dialects with no custom errors** use `Infallible`:

```rust
impl<I: Interpreter> Interpretable<I> for Arith<T> {
    type Error = Infallible;
    // InterpreterError from read() propagates via ? and From
}
```

**Dialects with custom errors** define their own error type:

```rust
enum ArithError { Overflow, DivisionByZero }

impl<I: Interpreter> Interpretable<I> for CheckedArith<T> {
    type Error = ArithError;
    // ArithError enters via Err(InterpError::Machine(ArithError::DivisionByZero))
}
```

## Composition via Lift

Same pattern as effects — `Lift` converts between `InterpError` types with different `ME` parameters:

```rust
impl<MEA, MEC> Lift<InterpError<MEA>> for InterpError<MEC>
where MEC: Lift<MEA>
{
    fn lift(from: InterpError<MEA>) -> Self {
        match from {
            InterpError::Interpreter(e) => InterpError::Interpreter(e),
            InterpError::Machine(me) => InterpError::Machine(Lift::lift(me)),
        }
    }
}
```

Composed dialect code uses `Lift::lift` on both effects and errors:

```rust
match self {
    Self::CheckedDiv(op) => {
        let effect = op.interpret(interp).map_err(Lift::lift)?;
        Ok(Lift::lift(effect))
    }
}
```
