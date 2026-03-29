# SingleStage Interpreter

The single-stage concrete interpreter. This is the initial focus — one language, one stage,
no fixpoint iteration.

## Effect Type

The concrete effect type for `SingleStage`:

```rust
enum Effect<V, Seed, DE> {
    // Cursor control
    Advance,
    Stay,
    Jump(Block, SmallVec<[V; 2]>),

    // Value binding
    BindValue(SSAValue, V),
    BindProduct(Product<ResultValue>, V),

    // Completion
    Return(V),
    Yield(V),
    Stop(V),

    // Composition
    Seq(SmallVec<[Self; 2]>),

    // Complex execution (seeds)
    Execute(Seed),

    // Dialect machine effects
    Machine(DE),
}
```

**Type parameters:**

- `V` — runtime value type (e.g., `i64`, `Value`)
- `Seed` — seed type for complex execution (e.g., `CompositeSeed<V>`). Dialects never construct
  `Execute` directly — they create seeds and execute them via `&mut I`.
- `DE` — dialect machine effect type. `()` for pure dialects, a custom enum for stateful dialects.

### Lift Implementation

See [effects.md](traits/effects.md#composition-via-lift) for the `Lift` impl between
`Effect` types with different `DE` parameters.

### Combinators

```rust
impl<V, Seed, DE> Effect<V, Seed, DE> {
    fn then(self, next: Self) -> Self {
        Self::Seq(smallvec![self, next])
    }
}
```

`Seq` composes any effects — base, machine, or mixed:

```rust
Effect::BindValue(result, v).then(Effect::Advance)

Effect::Seq(smallvec![
    Effect::BindValue(result, v),
    Effect::Machine(MemoryEffect::Flush),
    Effect::Advance,
])
```

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

Frames, cursors, and the call stack are **internal** — not part of the public trait API.

## Trait Implementations

### Machine

```rust
impl<V, M: Machine, S> Machine for SingleStage<V, M, S>
where CompositeSeed<V>: Execute<Self>,
{
    type Effect = Effect<V, CompositeSeed<V>, M::Effect>;
    type Error = InterpError<M::Error>;

    fn consume_effect(&mut self, effect: Self::Effect) -> Result<(), Self::Error> {
        match effect {
            Effect::Advance => self.advance_cursor(),
            Effect::Stay => Ok(()),
            Effect::Jump(block, args) => self.jump_to(block, args),
            Effect::BindValue(ssa, v) => self.values.bind(ssa, v).map_err(Into::into),
            Effect::BindProduct(results, v) => self.values.bind_product(results, v).map_err(Into::into),
            Effect::Return(v) => self.pop_frame_with(v),
            Effect::Yield(v) => self.yield_to_caller(v),
            Effect::Stop(v) => {
                self.result = Some(v);
                Ok(())
            }
            Effect::Seq(effects) => {
                for effect in effects {
                    self.consume_effect(effect)?;
                }
                Ok(())
            }
            Effect::Execute(seed) => {
                let terminal = seed.execute(self)?;
                self.consume_effect(terminal)
            }
            Effect::Machine(de) => {
                self.dialect_machine.consume_effect(de).map_err(InterpError::Machine)
            }
        }
    }
}
```

### ValueRead, PipelineAccess, ProjectMut

```rust
impl<V, M: Machine, S> ValueRead for SingleStage<V, M, S> {
    type Value = V;
    fn read(&self, value: SSAValue) -> Result<V, InterpreterError> { self.values.get(value) }
}

impl<V, M: Machine, S> PipelineAccess for SingleStage<V, M, S> {
    type StageInfo = S;
    fn pipeline(&self) -> &Pipeline<S> { &self.pipeline }
    fn current_stage(&self) -> CompileStage { self.current_stage }
}

impl<V, M: Machine, S, T> ProjectMut<T> for SingleStage<V, M, S>
where M: ProjectMut<T>,
{
    fn project_mut(&mut self) -> &mut T { self.dialect_machine.project_mut() }
}
```

### Interpreter

```rust
impl<V, M: Machine, S> Interpreter for SingleStage<V, M, S>
where CompositeSeed<V>: Execute<Self>,
{
    type Seed = CompositeSeed<V>;
    type DialectEffect = M::Effect;
    type DialectError = M::Error;

    fn step<L>(&mut self) -> Result<ControlFlow<V>, Self::Error>
    where
        L: Interpretable<Self>,
        M::Effect: Lift<L::Effect>,
        M::Error: Lift<L::Error>,
    {
        let statement = self.current_statement()?;
        let effect = statement.interpret(self)
            .map(Lift::lift)
            .map_err(Lift::lift)?;
        self.consume_effect(effect)?;

        // Check if Stop was processed (stored the value, returned Ok)
        if let Some(v) = self.result.take() {
            Ok(ControlFlow::Break(v))
        } else {
            Ok(ControlFlow::Continue(()))
        }
    }

    // run<L> uses the default impl: loop { match self.step::<L>()? { ... } }
}
```

## Execution Model

**`step<L>`** is the core execution method. It:
1. Fetches the current statement from the cursor
2. Calls `statement.interpret(self)` → `Effect<V, Seed, L::Effect>`
3. Lifts the effect via `Lift::lift` → `Effect<V, Seed, M::Effect>` (= `Self::Effect`)
4. Calls `self.consume_effect(effect)` to process it
5. Checks if `Stop(v)` was consumed (value stored in `self.result`)

**`run<L>`** uses the default impl from `Interpreter`: loop on `step` until `Break(v)`.

**Effect lifting:** When `L` is the top-level language (the common case), `L::Effect = M::Effect`
and `Lift::lift` is identity. For sub-dialects, `Lift` maps only the `Machine(de)` variant.

**Stop handling:** `consume_effect` for `Stop(v)` stores `v` in `self.result` and returns `Ok(())`.
On the next check in `step`, `self.result.take()` produces `Some(v)` → `Break(v)`.
