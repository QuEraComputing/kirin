# Error Model

Errors mirror the effect composition story: interpreter-shell failures and dialect-level
failures share a single outer wrapper.

## Base Interpreter Errors

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

These errors come from the interpreter shell: value lookup, stage resolution, frame handling,
and other execution-mechanics failures.

## Unified Error Type

```rust
enum InterpError<DE> {
    Interpreter(InterpreterError),
    Dialect(DE),
}
```

- `Interpreter(e)` is for shell failures.
- `Dialect(e)` is for dialect semantic failures and dialect-machine consumption failures.

`From<InterpreterError>` supports `?` on `read()` and other interpreter APIs:

```rust
impl<DE> From<InterpreterError> for InterpError<DE> {
    fn from(error: InterpreterError) -> Self {
        Self::Interpreter(error)
    }
}
```

## Recommended Conventions

- Dialects with no custom errors use `Infallible`.
- Dialects with custom semantic failures define a local error enum and return
  `Err(InterpError::Dialect(...))`.
- Interpreters route `Machine(de)` failures from the dialect machine into
  `InterpError::Dialect(...)`.

## Composition via Lift

Just like effects, only the dialect payload changes when composing:

```rust
impl<EA, EC> Lift<InterpError<EA>> for InterpError<EC>
where
    EC: Lift<EA>,
{
    fn lift(from: InterpError<EA>) -> Self {
        match from {
            InterpError::Interpreter(error) => InterpError::Interpreter(error),
            InterpError::Dialect(error) => InterpError::Dialect(Lift::lift(error)),
        }
    }
}
```

Composed dialect code stays uniform:

```rust
match self {
    Self::CheckedDiv(op) => {
        let effect = op.interpret(interp).map_err(Lift::lift)?;
        Ok(Lift::lift(effect))
    }
}
```

## Invariant

`InterpError<DE>` is the only public error envelope. A dialect or machine-specific failure
must never escape as a raw error type once it crosses the interpreter boundary.
