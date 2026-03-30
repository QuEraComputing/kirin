# SingleStage Interpreter

`SingleStage` is the first concrete interpreter for the design:

- one language
- one stage
- concrete execution
- no fixpoint loop

It is the reference implementation for the effect-first contract.

## Concrete Effect Type

```rust
enum Effect<V, DE> {
    Advance,
    Stay,
    Jump(Block, SmallVec<[V; 2]>),
    BindValue(SSAValue, V),
    BindProduct(Product<ResultValue>, V),
    Return(V),
    Yield(V),
    Stop(V),
    Seq(SmallVec<[Self; 2]>),
    Machine(DE),
}
```

There is no `Execute(Seed)` variant. Shared executors such as `BlockSeed`, `RegionSeed`,
and `FunctionSeed` are invoked directly and return ordinary outputs.

## Struct

```rust
struct SingleStage<V, M: Machine, S> {
    pipeline: Pipeline<S>,
    current_stage: CompileStage,
    values: ValueStore<V>,
    frames: FrameStack<V>,
    cursor: ExecutionCursor,
    dialect_machine: M,
    result: Option<V>,
}
```

Frames, cursors, and the call stack remain internal implementation details.

## Machine Implementation

```rust
impl<V, M: Machine, S> Machine for SingleStage<V, M, S> {
    type Effect = Effect<V, M::Effect>;
    type Error = InterpError<M::Error>;

    fn consume_effect(&mut self, effect: Self::Effect) -> Result<(), Self::Error> {
        match effect {
            Effect::Advance => self.advance_cursor(),
            Effect::Stay => Ok(()),
            Effect::Jump(block, args) => self.jump_to(block, args),
            Effect::BindValue(ssa, value) => self.values.bind(ssa, value).map_err(Into::into),
            Effect::BindProduct(results, value) => {
                self.values.bind_product(results, value).map_err(Into::into)
            }
            Effect::Return(value) => self.pop_frame_with(value),
            Effect::Yield(value) => self.yield_to_caller(value),
            Effect::Stop(value) => {
                self.result = Some(value);
                Ok(())
            }
            Effect::Seq(effects) => {
                for effect in effects {
                    self.consume_effect(effect)?;
                }
                Ok(())
            }
            Effect::Machine(effect) => {
                self.dialect_machine.consume_effect(effect).map_err(InterpError::Dialect)
            }
        }
    }
}
```

## Interpreter-Side Traits

```rust
impl<V, M: Machine, S> ValueRead for SingleStage<V, M, S> {
    type Value = V;

    fn read(&self, value: SSAValue) -> Result<V, InterpreterError> {
        self.values.get(value)
    }
}

impl<V, M: Machine, S> PipelineAccess for SingleStage<V, M, S> {
    type StageInfo = S;

    fn pipeline(&self) -> &Pipeline<S> { &self.pipeline }
    fn current_stage(&self) -> CompileStage { self.current_stage }
}
```

## Interpreter Implementation

```rust
impl<V, M: Machine, S> Interpreter for SingleStage<V, M, S> {
    type DialectEffect = M::Effect;
    type DialectError = M::Error;

    fn step<L>(&mut self) -> Result<ControlFlow<V>, Self::Error>
    where
        L: Interpretable<Self>,
        M::Effect: Lift<L::Effect>,
        M::Error: Lift<L::Error>,
    {
        let statement = self.current_statement()?;
        let effect = statement.interpret(self).map(Lift::lift).map_err(Lift::lift)?;
        self.consume_effect(effect)?;

        if let Some(value) = self.result.take() {
            Ok(ControlFlow::Break(value))
        } else {
            Ok(ControlFlow::Continue(()))
        }
    }
}
```

## Execution Model

`step<L>` performs the entire statement boundary:

1. fetch the current statement
2. interpret it
3. lift its effect into the composed dialect effect type
4. consume the lifted effect
5. detect whether `Stop(v)` completed execution

`run<L>` is the default loop over `step<L>`.

## Seed Interaction

Seeds do not pass through `consume_effect` as values. Instead:

- a dialect invokes a shared executor such as `BlockSeed` or `FunctionSeed`
- the seed may call `consume_effect` internally as it orchestrates nested execution
- the seed returns the smallest useful output to its caller, often a terminal effect or a
  regular dialect effect

This keeps `SingleStage` focused on one effect algebra while still allowing complex control flow.
