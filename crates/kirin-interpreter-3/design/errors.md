# Error Model

Errors follow the same Lift/Project algebra and GAT pattern as effects. Each interpreter defines its
error type as a GAT parameterized by the dialect's machine error type.

## Base Interpreter Errors

Shared error vocabulary that all interpreters support:

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

## Interpreter-Specific Errors (GAT)

Each interpreter type defines its error as a GAT parameterized by the dialect's machine error:

```rust
// SingleStage: base + machine errors
enum SingleStageError<ME> {
    Interpreter(InterpreterError),
    Machine(ME),
}

// Abstract: base + analysis-specific + machine errors
enum AbstractError<ME> {
    Interpreter(InterpreterError),
    FixpointDivergence,
    Machine(ME),
}
```

The GAT on the `Interpreter` trait:

```rust
trait Interpreter: Machine + ValueRead + PipelineAccess {
    type Error<ME>: TryLift<InterpreterError>
                  + TryLift<ME>;
    // ...
}
```

## Lifting Into Interpreter Errors

Same pattern as effects — `try_lift()` for uniform conversion:

```rust
// InterpreterError always lifts into interpreter errors
impl<ME> Lift<InterpreterError> for SingleStageError<ME> {
    fn lift(from: InterpreterError) -> Self { Self::Interpreter(from) }
}

// Machine errors always lift into interpreter errors
impl<ME> Lift<ME> for SingleStageError<ME> {
    fn lift(from: ME) -> Self { Self::Machine(from) }
}
```

## Dialect Error Usage

**Dialects with no custom errors** use `Infallible`:

```rust
impl<I: Interpreter> Interpretable<I> for Arith<T> {
    type Error = Infallible;  // no custom errors
    // InterpreterError from ctx.read() propagates via try_lift
}
```

**Dialects with custom errors** define their own error type:

```rust
enum ArithError { Overflow, DivisionByZero }

impl<I: Interpreter> Interpretable<I> for CheckedArith<T> {
    type Error = ArithError;
    // ArithError lifts into I::Error<ArithError> via Machine slot
}
```

## Composition: Lift Between GAT Instantiations

Same pattern as effects. If `ME2: Lift<ME>`, then `I::Error<ME2>: Lift<I::Error<ME>>`:

```rust
impl<ME, ME2> Lift<SingleStageError<ME>> for SingleStageError<ME2>
where ME2: Lift<ME>
{
    fn lift(from: SingleStageError<ME>) -> Self {
        match from {
            SingleStageError::Interpreter(e) => Self::Interpreter(e),
            SingleStageError::Machine(me) => Self::Machine(Lift::lift(me)),
        }
    }
}
```

Composed dialects use `try_lift()` for error conversion, same as effects.
