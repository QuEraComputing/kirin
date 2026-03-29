# Interpreter

The interpreter is a special Machine that doesn't belong to any dialect. It is the shell for the
entire IR — parameterized by a dialect machine, it handles everything the IR hard-codes:

**Interpreter responsibilities:**
- ValueStore: managing SSA values and their bindings
- Call stack / frames: push, pop, continuation management
- IR cursor: tracking the current statement position
- Execution loop: when to advance, stop, etc.

**NOT interpreter responsibilities:**
- Dialect-specific semantics (that's `Interpretable`)
- How to execute Block, Region, UnGraph, DiGraph (that's `Execute` on seed types)

Frames, cursors, and the call stack are **internal** — not part of the public trait API.

## SingleStage Implementation

```rust
struct SingleStage<V, M: Machine, S> {
    pipeline: Pipeline<S>,
    current_stage: CompileStage,
    values: ValueStore<V>,
    frames: FrameStack<V>,
    cursor: ExecutionCursor,
    dialect_machine: M,
}
```

### Trait Implementations

```rust
impl<V, M: Machine, S> Machine for SingleStage<V, M, S> {
    type Effect = SingleStageEffect<V, CompositeSeed<V>, M::Effect>;
    type Error = SingleStageError<M::Error>;

    fn consume_effect(&mut self, effect: Self::Effect) -> Result<(), Self::Error> {
        match effect {
            SingleStageEffect::Base(base) => self.consume_base(base),
            SingleStageEffect::Execute(seed) => {
                let terminal = seed.execute(self)?;
                self.consume_effect(terminal)
            }
            SingleStageEffect::Machine(de) => {
                self.dialect_machine.consume_effect(de).map_err(Lift::lift)
            }
        }
    }
}

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

impl<V, M: Machine, S> Interpreter for SingleStage<V, M, S> {
    type Effect<DE> = SingleStageEffect<V, CompositeSeed<V>, DE>;
    type Error<ME> = SingleStageError<ME>;
}
```

### Base Effect Consumption

```rust
fn consume_base(&mut self, effect: BaseEffect<V>) -> Result<(), Self::Error> {
    match effect {
        BaseEffect::Advance => self.advance_cursor(),
        BaseEffect::Stay => Ok(()),
        BaseEffect::Jump(block, args) => self.jump_to(block, args),
        BaseEffect::BindValue(ssa, v) => self.values.bind(ssa, v),
        BaseEffect::BindProduct(results, v) => self.values.bind_product(results, v),
        BaseEffect::Return(v) => self.pop_frame_with(v),
        BaseEffect::Yield(v) => self.yield_to_caller(v),
        BaseEffect::Stop(v) => self.stop_with(v),
        BaseEffect::Seq(effects) => {
            for effect in effects {
                self.consume_base(effect)?;
            }
            Ok(())
        }
    }
}
```

### Execution Loop

```rust
fn run<L>(&mut self) -> Result<V, Self::Error>
where L: Interpretable<Self>,
{
    loop {
        let statement = self.current_statement()?;
        let effect = statement.interpret(self)?;
        self.consume_effect(effect)?;
    }
}
```

`statement.interpret(self)` returns `I::Effect<L::Effect>` which equals `Machine::Effect`
when `L::Effect = M::Effect`. The dialect uses `try_lift()` internally; the interpreter loop
sees the concrete `SingleStageEffect` and dispatches.

## Use Cases

### Single-Stage Concrete Interpretation
**This is the initial focus.** The language is simple, parameterized on a particular language
and compile-stage.

### Multi-Stage Concrete Interpretation (Deferred)
Dynamic dispatch on stages. The loop may switch between stages and languages.

### Abstract Interpretation (Deferred)
Fixpoint execution. `AbstractEffect` adds `Fork` for nondeterministic branches.
Widening/narrowing for termination guarantees.
