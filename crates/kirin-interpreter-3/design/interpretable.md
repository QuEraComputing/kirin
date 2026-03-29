# Interpretable Trait

The `Interpretable` trait is the dialect author's primary interface for providing dialect-specific semantics.

## Trait Definition

```rust
trait Interpretable<I: Interpreter> {
    type Effect;    // dialect's own machine effects (() or Infallible if none)
    type Error;     // dialect's own errors (Infallible if none)

    fn interpret(&self, interp: &mut I) -> Result<I::Effect<Self::Effect>, I::Error<Self::Error>>;
}
```

- **`I: Interpreter`** — dialects specialize on interpreter types because operational semantics
  depend on the interpreter (concrete vs abstract vs symbolic).
- **`&mut I`** — full interpreter access. Value reading via `ValueRead`, pipeline access via
  `PipelineAccess`, machine projection via `ProjectMut<M>`.
- **`I::Effect<Self::Effect>`** — the interpreter's effect GAT parameterized by the dialect's own
  machine effect type. Dialects construct this via `try_lift()`.
- **`I::Error<Self::Error>`** — symmetric: interpreter's error GAT parameterized by dialect's error type.

## Interpreter Supertrait

```rust
trait Interpreter: Machine + ValueRead + PipelineAccess {
    type Effect<DE>: TryLift<BaseEffect<<Self as ValueRead>::Value>>
                   + TryLift<DE>;
    type Error<ME>: TryLift<InterpreterError>
                  + TryLift<ME>;
}
```

Sub-traits:

```rust
trait ValueRead {
    type Value: Clone;
    fn read(&self, value: SSAValue) -> Result<Self::Value, InterpreterError>;
}

trait PipelineAccess {
    type StageInfo;
    fn pipeline(&self) -> &Pipeline<Self::StageInfo>;
    fn current_stage(&self) -> CompileStage;
    fn resolve_callee(
        &self,
        function: Function,
        args: &[Self::Value],
        policy: ResolutionPolicy,
    ) -> Result<SpecializedFunction, InterpreterError>
    where Self: ValueRead;
}
```

`ProjectMut<T>` is per-dialect — not all dialects need machine projection. See [examples.md](examples.md).

## Key Differences from interpreter-1/2

- **No `'ir` lifetime on the trait.** Handled internally by the interpreter.
- **No `L` type parameter.** No E0275 recursive trait resolution cycle.
- **GAT-based effects/errors.** `I::Effect<DE>` and `I::Error<ME>` parameterize the interpreter's
  types by the dialect's own types. Uniform `try_lift()` for all conversions.
- **Unified Lift/Project algebra.** One composition mechanism for everything.
