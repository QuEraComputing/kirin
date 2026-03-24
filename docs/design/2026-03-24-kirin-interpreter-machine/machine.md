# Machine

## Core Split

This design uses a strict split between:

- semantic machines
- interpreter shells

A `Machine<'ir>` is dialect- or language-defined semantic state. It is not the
interpreter shell.

An `Interpreter<'ir>` is the typed shell over one top-level machine. It owns:

- the internal cursor stack
- current execution location
- control consumption
- step and run driver loops
- shell-level suspension policy such as breakpoints and fuel

The shell does not define language semantics such as:

- call frames
- return conventions
- yield conventions
- loop stacks
- graph traversal stacks
- product packing or unpacking policy

Those stay on dialect-defined machine types and effect types.

## Semantic Machine Traits

The structural machine trait should stay thin:

```rust
trait Machine<'ir> {
    type Effect;
    type Stop;
}
```

Effects and semantic stop payloads compose structurally with machine
composition.

The two primary behavior traits are:

```rust
trait Interpretable<'ir, I>: Dialect
where
    I: Interpreter<'ir>,
{
    type Machine: Machine<'ir>;
    type Error;

    fn interpret(
        &self,
        interp: &mut I,
    ) -> Result<<Self::Machine as Machine<'ir>>::Effect, Self::Error>;
}
```

```rust
trait ConsumeEffect<'ir>: Machine<'ir> {
    type Error;

    fn consume_effect(
        &mut self,
        effect: Self::Effect,
    ) -> Result<Control<Self::Stop>, Self::Error>;
}
```

Both traits stay minimal:

- no projection bounds are baked into the traits
- no lifting bounds are baked into the traits
- local behavior owns its own local error type

Those bounds are added only on the interpreter forwarding helpers or on
individual impl blocks that need them.

## Structural Machine Composition

Composed machine types should provide explicit structural traits:

- `ProjectMachine<T>`
- `ProjectMachineMut<T>`
- `LiftEffect<'ir, Sub>`
- `LiftStop<'ir, Sub>`

These traits live on composed machine types, not on the interpreter shell.
The shell forwards them for ergonomics.

This gives the intended composition rule:

```rust
enum DialectC {
    A(DialectA),
    B(DialectB),
}

struct MachineC {
    a: MachineA,
    b: MachineB,
}

enum EffectC {
    A(EffectA),
    B(EffectB),
}

enum StopC {
    A(StopA),
    B(StopB),
}
```

The important symmetry is:

- dialect composition is sum-like
- machine composition is product-like
- effect composition is sum-like
- stop composition is sum-like

## Shell Control

The shell-facing control language is:

```rust
enum Control<Stop> {
    Advance,
    Stay,
    Push(ExecutionSeed),
    Replace(ExecutionSeed),
    Pop,
    Stop(Stop),
}
```

Meaning:

- `Advance`
  Move to the next statement in the current execution context.
- `Stay`
  Keep the current cursor unchanged after semantic-state updates.
- `Push(seed)`
  Start nested execution by pushing a new execution context.
- `Replace(seed)`
  Replace the current execution context with a new one.
- `Pop`
  Finish the current execution context and resume its parent.
- `Stop(stop)`
  Stop execution for a semantic reason defined by the top-level machine.

`Control<Stop>` is a plain public enum. The invariants live in
`ExecutionSeed`, not in `Control` itself.

`Control` should provide one small helper:

```rust
impl<S> Control<S> {
    fn map_stop<T>(self, f: impl FnOnce(S) -> T) -> Control<T>;
}
```

This makes composite machine effect consumption concise:

```rust
self.a.consume_effect(effect_a)?
    .map_stop(StopC::A)
```

## Execution Seeds

The shell keeps full cursors internal. Public code constructs execution seeds.

Seeds are strictly intra-stage. Cross-stage execution is a separate interpreter
capability and is never encoded in `Control`.

The public seed surface should be:

```rust
pub enum ExecutionSeed {
    Block(BlockSeed),
    Region(RegionSeed),
    DiGraph(DiGraphSeed),
    UnGraph(UnGraphSeed),
}
```

with named per-shape seed types:

- `BlockSeed`
- `RegionSeed`
- `DiGraphSeed`
- `UnGraphSeed`

These seed types should be public but use private fields and constructor
helpers. CFG seeds can stay simple. Graph seeds may need richer entry payloads.

The framework may later extend this surface with branch fan-out support, but v1
keeps `Control` single-seed.

## Interpreter Shell Trait

The typed shell contract is:

```rust
trait Interpreter<'ir>: ValueStore + StageAccess<'ir> {
    type Machine: Machine<'ir> + ConsumeEffect<'ir>;
    type Error;

    fn machine(&self) -> &Self::Machine;
    fn machine_mut(&mut self) -> &mut Self::Machine;
}
```

`Interpreter<'ir>` is the full typed shell contract. It should own:

- top-level machine access
- projection/lifting forwarding helpers
- interpret APIs
- effect-consumption APIs
- control-consumption APIs
- `step`
- `run`
- `run_until_break`

Fuel and breakpoints stay on separate sibling traits rather than becoming
supertraits of `Interpreter<'ir>`.

The shell should forward structural machine operations for ergonomics:

- `project_machine::<Sub>()`
- `project_machine_mut::<Sub>()`
- `lift_effect::<Sub>(...)`
- `lift_stop::<Sub>(...)`

## Interpret And Consume APIs

The typed interpreter surface should distinguish local and top-level APIs.

Interpret:

- `interpret_local(stmt)`
  Returns the local machine effect for `stmt`.
- `interpret_lifted(stmt)`
  Returns the lifted top-level machine effect.
- `interpret_current()`
  Cursor-driven API. Returns the top-level machine effect of the current
  statement.

Consume:

- `consume_local_effect(effect)`
  Consumes a submachine effect against only the projected submachine and returns
  `Control<Sub::Stop>`.
- `consume_lifted_effect(effect)`
  Lifts a local effect into the top-level machine effect and consumes it as the
  full machine.
- `consume_effect(effect)`
  Consumes a top-level machine effect and returns `Control<I::Machine::Stop>`.

Control:

- `consume_local_control(control)`
  Convenience helper. Lifts `Control<Sub::Stop>` into top-level control and
  forwards to `consume_control`.
- `consume_control(control)`
  Consumes shell control against the interpreter shell itself.

All interpreter forwarding methods use method-level conversion bounds such as:

- `D::Error: Into<I::Error>`
- `Sub::Error: Into<I::Error>`

The shell owns the final error surface. Local dialect and machine logic keep
their own local error types.

## Driver Result Types

The driver-level result types should be:

```rust
struct StepResult<E, S> {
    effect: E,
    control: Control<S>,
}
```

```rust
enum StepOutcome<E, S> {
    Stepped(StepResult<E, S>),
    Suspended(SuspendReason),
    Completed,
}
```

```rust
enum RunResult<S> {
    Stopped(S),
    Suspended(SuspendReason),
    Completed,
}
```

```rust
enum SuspendReason {
    Breakpoint,
    FuelExhausted,
    HostInterrupt,
}
```

`step()` is a driver-style API:

- `Completed` when there is no current statement
- `Suspended(...)` for shell-level suspension
- `Stepped(...)` when one statement was executed

`interpret_current()` is lower-level and errors if there is no current
statement.

## Breakpoints And Locations

The public execution-location surface stays statement-oriented:

```rust
enum ExecutionLocation {
    BeforeStatement(Statement),
    AfterStatement(Statement),
}
```

Dynamic breakpoints are keyed by:

- stage
- execution location

`BreakpointControl` should work over a dedicated value object:

```rust
struct Breakpoint {
    stage: CompileStage,
    location: ExecutionLocation,
}
```

In v1, breakpoints are plain value objects:

- add by value
- remove by value
- query membership by value

The docs should explicitly distinguish:

- shell breakpoint
  debugger/driver suspension
- semantic breakpoint statement
  language-defined stop or effect

## Internal Cursor Stack

The shell owns a stack of execution cursors.

This is not a semantic call stack. It is only the generic nesting stack for
execution contexts. Dialects may keep semantic frame data in their own machine
state if they need it.

The split is:

- shell cursor stack
  - where execution currently is
  - what nested execution contexts are active
- dialect-owned machine state
  - what that nesting means semantically

This allows dialects to define call stacks, graph traversal stacks, or loop
stacks without forcing one framework-wide frame model.

## Step Lifecycle

The common lifted shell cycle is:

1. resolve the current statement from the top cursor
2. interpret it into the top-level machine effect
3. consume that effect through the top-level machine
4. obtain `Control<I::Machine::Stop>`
5. consume that control on the interpreter shell

The local testing path uses the same phases with local effects and local
control.

## Default Body Runners

Because statements own body execution semantics, body runners are helper
facilities, not semantic authorities.

The framework should provide explicit default helpers such as:

- `DefaultBlockRunner`
- `DefaultCFGRegionRunner`

These helpers are optional reusable execution strategies for statements that
want standard CFG behavior. They do not define the meaning of `Block` or
`Region` globally.

Future graph helpers should follow the same naming rule: explicit default
execution strategies, not universal graph semantics.
