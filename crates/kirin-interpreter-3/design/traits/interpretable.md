# Interpretable Trait

`Interpretable<I>` is the dialect author's primary interface for statement semantics.

## Trait Definition

```rust
trait Interpretable<I: Interpreter> {
    type Effect;
    type Error;

    fn interpret(
        &self,
        interp: &mut I,
    ) -> Result<Effect<I::Value, Self::Effect>, InterpError<Self::Error>>;
}
```

## Contract

- `I: Interpreter` keeps semantics specialized to the interpreter model.
- `&mut I` gives access to interpreter services such as value reads, callee resolution,
  and reusable sub-executors like block, region, and function execution.
- The return type is always the unified effect algebra.
- Dialects with no machine effects use `type Effect = Infallible`.
- Dialects with no custom errors use `type Error = Infallible`.

## What `interpret()` May Do

- Read SSA values via `ValueRead`
- Resolve callees or stages via `PipelineAccess`
- Invoke shared seeds such as `BlockSeed`, `RegionSeed`, or `FunctionSeed`
- Return `Effect<V, DE>` values

## What `interpret()` Must Not Do

- Advance the cursor directly
- Push or pop frames directly
- Mutate dialect machine state directly
- Bypass the effect algebra for semantically visible work

If a state change matters, return `Effect::Machine(de)`.

## Interpreter Supertrait

```rust
trait Interpreter: Machine + ValueRead + PipelineAccess {
    type DialectEffect;
    type DialectError;

    fn step<L>(&mut self) -> Result<ControlFlow<Self::Value>, Self::Error>
    where
        L: Interpretable<Self>,
        Self::DialectEffect: Lift<L::Effect>,
        Self::DialectError: Lift<L::Error>;

    fn run<L>(&mut self) -> Result<Self::Value, Self::Error>
    where
        L: Interpretable<Self>,
        Self::DialectEffect: Lift<L::Effect>,
        Self::DialectError: Lift<L::Error>,
    {
        loop {
            match self.step::<L>()? {
                ControlFlow::Continue(()) => continue,
                ControlFlow::Break(value) => return Ok(value),
            }
        }
    }
}
```

For any interpreter:

- `Machine::Effect = Effect<Self::Value, Self::DialectEffect>`
- `Machine::Error = InterpError<Self::DialectError>`

`step` is the only required execution entry point. It performs:

1. Fetch current statement
2. Call `interpret`
3. Lift the returned effect and error into the interpreter's composed dialect types
4. Consume the lifted effect
5. Report whether execution has completed

## Supporting Traits

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
    where
        Self: ValueRead;
}
```

## Key Differences from interpreter-1/2

- No public frame or cursor APIs
- No direct-mutation escape hatch for dialects
- No seed variant inside `Effect`
- No GAT-based effect family
- One composition story for both effects and errors
