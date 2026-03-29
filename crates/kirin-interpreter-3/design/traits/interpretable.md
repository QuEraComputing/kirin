# Interpretable Trait

The `Interpretable` trait is the dialect author's primary interface for providing dialect-specific semantics.

## Trait Definition

```rust
trait Interpretable<I: Interpreter> {
    type Effect;    // dialect's own machine effects (() if none)
    type Error;     // dialect's own errors (Infallible if none)

    fn interpret(&self, interp: &mut I)
        -> Result<Effect<I::Value, I::Seed, Self::Effect>, InterpError<Self::Error>>;
}
```

- **`I: Interpreter`** — dialects specialize on interpreter types because operational semantics
  depend on the interpreter (concrete vs abstract vs symbolic).
- **`&mut I`** — full interpreter access. Value reading via `ValueRead`, pipeline access via
  `PipelineAccess`, machine projection via `ProjectMut<M>`.
- **`Effect<I::Value, I::Seed, Self::Effect>`** — the unified effect type, parameterized by the
  dialect's own machine effect type. Dialects construct variants directly (`Effect::Advance`,
  `Effect::BindValue(...)`, `Effect::Machine(...)`).
- **`InterpError<Self::Error>`** — the unified error type. `InterpreterError` from `read()`
  propagates via `?` (using `From`). Custom errors use `InterpError::Machine(...)`.

## Interpreter Supertrait

```rust
trait Interpreter: Machine + ValueRead + PipelineAccess {
    type Seed;
    type DialectEffect;
    type DialectError;

    /// Execute one statement: interpret it, lift the effect, consume it.
    /// Returns `Continue(())` to keep going, `Break(v)` when execution completes.
    fn step<L>(&mut self) -> Result<ControlFlow<Self::Value>, Self::Error>
    where
        L: Interpretable<Self>,
        Self::DialectEffect: Lift<L::Effect>,
        Self::DialectError: Lift<L::Error>;

    /// Run until completion. Default: loop on `step`.
    fn run<L>(&mut self) -> Result<Self::Value, Self::Error>
    where
        L: Interpretable<Self>,
        Self::DialectEffect: Lift<L::Effect>,
        Self::DialectError: Lift<L::Error>,
    {
        loop {
            match self.step::<L>()? {
                ControlFlow::Continue(()) => continue,
                ControlFlow::Break(v) => return Ok(v),
            }
        }
    }
}
```

The `Interpreter` trait extends `Machine` — the interpreter IS a machine whose effect type is
`Effect<Self::Value, Self::Seed, Self::DialectEffect>`:

- `Machine::Effect = Effect<Self::Value, Self::Seed, Self::DialectEffect>`
- `Machine::Error = InterpError<Self::DialectError>`

**`step`** is the required method — concrete implementations provide the interpret → lift → consume
pipeline. **`run`** has a default impl that loops on `step`; override for custom strategies
(e.g., abstract interpretation fixpoint).

`consume_effect` comes from `Machine`, value access from `ValueRead`, pipeline from `PipelineAccess`.
Seeds compose via `Execute` (e.g., `IfSeed` creates a `BlockSeed` and calls `.execute(interp)`).

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

`ProjectMut<T>` is per-dialect — not all dialects need machine projection. See [examples](../examples/index.md).

## Key Differences from interpreter-1/2

- **No `'ir` lifetime on the trait.** Handled internally by the interpreter.
- **No `L` type parameter.** No E0275 recursive trait resolution cycle.
- **No GATs.** The unified `Effect<V, Seed, DE>` type replaces GAT-based `I::Effect<DE>`.
  Dialects construct effect variants directly — no `try_lift()`.
- **Unified Lift algebra.** `Lift<Effect<V, S, DEA>> for Effect<V, S, DEC>` handles composition.
