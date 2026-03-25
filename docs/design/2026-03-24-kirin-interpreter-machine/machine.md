# Machine

## Core Split

This design uses a strict split between:

- semantic machines
- interpreter shells

A `Machine<'ir>` is dialect- or language-defined semantic state. It is not the
interpreter shell.

An `Interpreter<'ir>` is the typed shell over one top-level machine. It owns:

- top-level machine access
- control consumption
- typed interpret and effect-consumption APIs

The shell does not define language semantics such as:

- call frames
- return conventions
- yield conventions
- loop stacks
- graph traversal stacks
- product packing or unpacking policy

Those stay on dialect-defined machine types and effect types.

Concrete shells and typed stage views may additionally expose:

- `interpreter::Position<'ir>` for read-only cursor inspection
- `interpreter::Driver<'ir>` for step/run loops and suspension policy

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
    ) -> Result<Shell<Self::Stop>, Self::Error>;
}
```

Both traits stay minimal:

- no projection bounds are baked into the traits
- no lifting bounds are baked into the traits
- local behavior owns its own local error type
- downstream dialect authors implementing `Interpretable` only depend on
  `Interpreter<'ir>`, not on driver or position traits

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

The intended signatures are:

```rust
trait ProjectMachine<T: ?Sized> {
    fn project(&self) -> &T;
}

trait ProjectMachineMut<T: ?Sized> {
    fn project_mut(&mut self) -> &mut T;
}

trait LiftEffect<'ir, Sub>: Machine<'ir>
where
    Sub: Machine<'ir>,
{
    fn lift_effect(effect: Sub::Effect) -> Self::Effect;
}

trait LiftStop<'ir, Sub>: Machine<'ir>
where
    Sub: Machine<'ir>,
{
    fn lift_stop(stop: Sub::Stop) -> Self::Stop;
}
```

Projection allows `?Sized` to leave room for future borrowed or erased machine
views. Effect and stop lifting stay concrete and sized because they describe
structural composition of real machine components.

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
enum Shell<Stop> {
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

`control::Shell<Stop>` is a plain public enum. The invariants live in
`ExecutionSeed`, not in `Shell` itself.

`Shell` should provide one small helper:

```rust
impl<S> Shell<S> {
    fn map_stop<T>(self, f: impl FnOnce(S) -> T) -> Shell<T>;
}
```

This makes composite machine effect consumption concise:

```rust
self.a.consume_effect(effect_a)?
    .map_stop(StopC::A)
```

## Shell Control Invariants

The shell should keep the control invariants strict:

- `Push(seed)` may grow the cursor stack.
- `Replace(seed)` requires an active current cursor.
- `Pop` requires an active current cursor.
- `Stop(stop)` clears the cursor stack immediately.
- invalid control against empty execution state is an interpreter error.

`Completed` is a driver outcome, not a control-application fallback. So:

- empty-stack `Pop` is an interpreter error
- empty-stack `Replace` is an interpreter error

`Advance` is cursor-kind specific. The public control layer does not define one
universal “advance at end” rule. Advancing within a block, region, or graph
cursor is part of internal cursor semantics.

`Stay` means “no intended execution movement”, but the shell may still run
consistency checks after applying it.

## Execution Seeds

The shell keeps full cursors internal. Public code constructs execution seeds.

Seeds are strictly intra-stage. Cross-stage execution is a separate interpreter
capability and is never encoded in `Shell`.

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
keeps `Shell` single-seed.

### MVP Checkpoint

The current `kirin-interpreter-2` MVP now exposes the full public seed family:

- `ExecutionSeed::Block`
- `ExecutionSeed::Region`
- `ExecutionSeed::DiGraph`
- `ExecutionSeed::UnGraph`
- `BlockSeed`
- `RegionSeed`
- `DiGraphSeed`
- `UnGraphSeed`

The single-stage shell also uses a closed internal cursor enum over those same
four execution shapes.

## Interpreter Shell Trait

The typed shell contract is:

```rust
trait Interpreter<'ir>: ValueStore + StageAccess<'ir> {
    type Machine: Machine<'ir> + ConsumeEffect<'ir>;
    type Error;

    fn machine(&self) -> &Self::Machine;
    fn machine_mut(&mut self) -> &mut Self::Machine;

    fn interpret_current(
        &mut self,
    ) -> Result<<Self::Machine as Machine<'ir>>::Effect, Self::Error>;

    fn consume_effect(
        &mut self,
        effect: <Self::Machine as Machine<'ir>>::Effect,
    ) -> Result<Shell<<Self::Machine as Machine<'ir>>::Stop>, Self::Error>;

    fn consume_control(
        &mut self,
        control: Shell<<Self::Machine as Machine<'ir>>::Stop>,
    ) -> Result<(), Self::Error>;
}
```

`Interpreter<'ir>` is the full typed shell contract. It should own:

- top-level machine access
- projection/lifting forwarding helpers
- interpret APIs
- effect-consumption APIs
- control-consumption APIs

It should not require:

- current execution location
- cursor-depth inspection
- driver loops
- breakpoint, fuel, or interrupt policy

Fuel and breakpoints stay on separate public traits rather than becoming part
of `Interpreter<'ir>` itself.

The required primitive shell methods are:

- `machine`
- `machine_mut`
- `interpret_current`
- `consume_effect`
- `consume_control`

Everything else can be a provided default layered on top of those primitives.

The shell should forward structural machine operations for ergonomics:

- `project_machine::<Sub>()`
- `project_machine_mut::<Sub>()`
- `lift_effect::<Sub>(...)`
- `lift_stop::<Sub>(...)`

These should be provided forwarding methods, not extra implementation burden:

```rust
fn project_machine<Sub: ?Sized>(&self) -> &Sub
where
    Self::Machine: ProjectMachine<Sub>,
{
    self.machine().project()
}

fn project_machine_mut<Sub: ?Sized>(&mut self) -> &mut Sub
where
    Self::Machine: ProjectMachineMut<Sub>,
{
    self.machine_mut().project_mut()
}

fn lift_effect<Sub: Machine<'ir>>(
    &self,
    effect: Sub::Effect,
) -> <Self::Machine as Machine<'ir>>::Effect
where
    Self::Machine: LiftEffect<'ir, Sub>,
{
    <Self::Machine as LiftEffect<'ir, Sub>>::lift_effect(effect)
}

fn lift_stop<Sub: Machine<'ir>>(
    &self,
    stop: Sub::Stop,
) -> <Self::Machine as Machine<'ir>>::Stop
where
    Self::Machine: LiftStop<'ir, Sub>,
{
    <Self::Machine as LiftStop<'ir, Sub>>::lift_stop(stop)
}
```

## Position Trait

Typed shells and typed stage views should share one small read-only cursor
inspection trait:

```rust
trait Position<'ir>: StageAccess<'ir> {
    fn cursor_depth(&self) -> usize;
    fn current_block(&self) -> Option<Block>;
    fn current_statement(&self) -> Option<Statement>;
    fn current_location(&self) -> Option<Location>;
}
```

The intent is:

- `current_statement()`
  the statement that would execute next if execution proceeds immediately
- `current_location()`
  the breakpoint-facing location, including post-step
  `Location::AfterStatement(_)` checkpoints
- `current_block()` and `cursor_depth()`
  read-only structural inspection for tests, tooling, and typed stage views

`Position<'ir>` is not a semantic dependency of `Interpretable`. It is a
shell/view observation trait.

## Driver Trait

The high-level stepping surface should live in a second layered trait rather
than on `Interpreter<'ir>` directly:

```rust
trait Driver<'ir>:
    Interpreter<'ir>
    + Position<'ir>
    + control::Fuel
    + control::Breakpoints
    + control::Interrupt
{
    fn poll_execution_gate(
        &mut self,
    ) -> Result<Option<Statement>, Suspension>;

    fn stop_pending(&self) -> bool;

    fn take_stop(
        &mut self,
    ) -> Option<<Self::Machine as Machine<'ir>>::Stop>;

    fn finish_step(&mut self, statement: Statement);

    fn step(
        &mut self,
    ) -> Result<
        Step<
            <Self::Machine as Machine<'ir>>::Effect,
            <Self::Machine as Machine<'ir>>::Stop,
        >,
        Self::Error,
    >
    where
        <Self::Machine as Machine<'ir>>::Effect: Clone,
        Shell<<Self::Machine as Machine<'ir>>::Stop>: Clone;

    fn run(
        &mut self,
    ) -> Result<Run<<Self::Machine as Machine<'ir>>::Stop>, Self::Error>;

    fn run_until_break(
        &mut self,
    ) -> Result<Run<<Self::Machine as Machine<'ir>>::Stop>, Self::Error>;
}
```

Only the shell-facing hooks are required. `step()`, `run()`, and
`run_until_break()` are intended to be provided defaults layered on top of
those hooks plus the `Interpreter<'ir>` primitives.

The required hooks stay small and shell-facing:

- `poll_execution_gate()`
  resolves the next executable statement while applying suspension policy and
  clearing any stale post-step checkpoint
- `stop_pending()` / `take_stop()`
  expose the shell's latched semantic stop slot
- `finish_step(statement)`
  records post-step state such as `Location::AfterStatement(statement)` when no
  semantic stop was latched

This is the right boundary for the current roadmap:

- `interpreter::SingleStage<L>` already has inherent equivalents for these
  hooks
- future typed stage views can forward the same hooks into shared dynamic-shell
  state
- ordinary dialect semantics stay on `Interpreter<'ir>` alone

## Driver Control Traits

Fuel and breakpoints are shell-driver concerns. They are not part of
`Machine<'ir>` composition and should not be stored in user-composed semantic
machine state.

The intended control traits are:

```rust
trait Breakpoints {
    fn add_breakpoint(&mut self, breakpoint: Breakpoint) -> bool;
    fn remove_breakpoint(&mut self, breakpoint: &Breakpoint) -> bool;
    fn has_breakpoint(&self, breakpoint: &Breakpoint) -> bool;
}

trait Fuel {
    fn fuel(&self) -> Option<u64>;
    fn set_fuel(&mut self, fuel: Option<u64>);
    fn add_fuel(&mut self, fuel: u64);
}

trait Interrupt {
    fn request_interrupt(&mut self);
    fn clear_interrupt(&mut self);
    fn interrupt_requested(&self) -> bool;
}
```

The behavioral rules are:

- breakpoints are plain value objects
- `add_breakpoint` / `remove_breakpoint` return whether the set changed
- `add_fuel` saturates on overflow
- fuel is a shell progress budget, not a semantic machine resource
- `None` fuel means unlimited
- `Some(0)` fuel is legal and suspends before the next statement executes
- burning the last unit of fuel still lets that statement return `Stepped(...)`
- `HostInterrupt` is level-triggered and remains active until explicitly cleared

These traits should be implemented on:

- typed interpreters and typed stage views
- the stage-dynamic shell

`interpreter::Driver<'ir>` may inherit these traits, but `Interpreter<'ir>`
should not.

For the dynamic shell:

- breakpoint state is one shared shell-level breakpoint set
- fuel is one shared shell-level driver counter
- typed stage views forward into that shared shell state
- host interrupt is one shared shell-level latched flag

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
  `Shell<Sub::Stop>`.
- `consume_lifted_effect(effect)`
  Lifts a local effect into the top-level machine effect and consumes it as the
  full machine.
- `consume_effect(effect)`
  Consumes a top-level machine effect and returns `Shell<I::Machine::Stop>`.

Shell:

- `consume_local_control(control)`
  Convenience helper. Lifts `Shell<Sub::Stop>` into top-level control and
  forwards to `consume_control`.
- `consume_control(control)`
  Consumes shell control against the interpreter shell itself.

The intended method shapes are:

```rust
fn interpret_local<D>(
    &mut self,
    stmt: &D,
) -> Result<<D::Machine as Machine<'ir>>::Effect, Self::Error>
where
    D: Interpretable<'ir, Self>,
    D::Error: Into<Self::Error>;

fn interpret_lifted<D>(
    &mut self,
    stmt: &D,
) -> Result<<Self::Machine as Machine<'ir>>::Effect, Self::Error>
where
    D: Interpretable<'ir, Self>,
    Self::Machine: LiftEffect<'ir, D::Machine>,
    D::Error: Into<Self::Error>;

fn consume_local_effect<Sub: Machine<'ir> + ConsumeEffect<'ir>>(
    &mut self,
    effect: <Sub as Machine<'ir>>::Effect,
) -> Result<Shell<<Sub as Machine<'ir>>::Stop>, Self::Error>
where
    Self::Machine: ProjectMachineMut<Sub>,
    <Sub as ConsumeEffect<'ir>>::Error: Into<Self::Error>;

fn consume_lifted_effect<Sub: Machine<'ir>>(
    &mut self,
    effect: <Sub as Machine<'ir>>::Effect,
) -> Result<Shell<<Self::Machine as Machine<'ir>>::Stop>, Self::Error>
where
    Self::Machine: LiftEffect<'ir, Sub>,
    <Self::Machine as ConsumeEffect<'ir>>::Error: Into<Self::Error>;

fn consume_local_control<Sub: Machine<'ir>>(
    &mut self,
    control: Shell<<Sub as Machine<'ir>>::Stop>,
) -> Result<(), Self::Error>
where
    Self::Machine: LiftStop<'ir, Sub>;
```

The default layering should be:

- `interpret_lifted`
  = `interpret_local` + `lift_effect`
- `consume_lifted_effect`
  = `lift_effect` + `consume_effect`
- `consume_local_control`
  = `map_stop` + `lift_stop` + `consume_control`

Low-level typed APIs ignore shell suspension policy:

- `interpret_current`
- `interpret_local`
- `interpret_lifted`
- `consume_local_effect`
- `consume_lifted_effect`
- `consume_effect`
- `consume_local_control`
- `consume_control`

Only driver APIs apply shell suspension policy:

- `step`
- `run`
- `run_until_break`

All interpreter forwarding methods use method-level conversion bounds such as:

- `D::Error: Into<I::Error>`
- `Sub::Error: Into<I::Error>`

The shell owns the final error surface. Local dialect and machine logic keep
their own local error types.

## Driver Result Types

The driver-level result types should be:

```rust
struct Stepped<E, S> {
    effect: E,
    control: Shell<S>,
}
```

```rust
enum Step<E, S> {
    Stepped(Stepped<E, S>),
    Suspended(Suspension),
    Completed,
}
```

```rust
enum Run<S> {
    Stopped(S),
    Suspended(Suspension),
    Completed,
}
```

```rust
enum Suspension {
    Breakpoint,
    FuelExhausted,
    HostInterrupt,
}
```

`Driver::step()` is a driver-style API:

- `Completed` when there is no current statement
- `Suspended(...)` for shell-level suspension
- `Stepped(...)` when one statement was executed

`interpret_current()` is lower-level and errors if there is no current
statement.

`Driver::step()` should be a provided method when the artifacts it returns are
cloneable. The clean condition is:

- `<Self::Machine as Machine<'ir>>::Effect: Clone`
- `Shell<<Self::Machine as Machine<'ir>>::Stop>: Clone`

That default can:

1. check shell suspension policy
2. interpret the current statement
3. consume the resulting top-level effect
4. clone the effect/control it needs to return
5. consume the control on the shell
6. return `Step::Stepped(...)`

Concrete shells and typed stage views may override this path if they want a
cheaper move-based implementation.

`Driver::run()` and `Driver::run_until_break()` should also be provided
defaults, but they should loop directly over:

- `interpret_current`
- `consume_effect`
- `consume_control`

rather than depending on `step()`. This avoids inheriting the clone bounds of
the default `step()`.

`Driver::run_until_break()` should:

- return `Suspended(Breakpoint)` immediately if a breakpoint is already active
  at the current stage/location
- return `Suspended(Breakpoint)` when a later step reaches a breakpoint
- still stop on any other suspension reason instead of hiding it

Driver APIs should share one immediate suspension priority order:

1. breakpoint at current location
2. fuel exhausted
3. host interrupt

Fuel should be decremented only for successful statement execution:

- breakpoint checks do not burn fuel
- immediate suspension does not burn fuel
- one fully executed statement burns one fuel unit

If the last statement executes and leaves the cursor stack empty, that call to
`Driver::step()` should still return `Stepped(...)`. `Completed` means no
statement executed because execution was already exhausted.

## Breakpoints And Locations

The public execution-location surface stays statement-oriented:

```rust
enum Location {
    BeforeStatement(Statement),
    AfterStatement(Statement),
}
```

Dynamic breakpoints are keyed by:

- stage
- execution location

`control::Breakpoints` should work over a dedicated value object:

```rust
struct Breakpoint {
    stage: CompileStage,
    location: Location,
}
```

In v1, breakpoints are plain value objects:

- add by value
- remove by value
- query membership by value

### MVP Checkpoint

The implemented single-stage shell supports both `BeforeStatement` and
`AfterStatement` breakpoints by keeping an internal post-step checkpoint
between successful statement executions.

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

The common lifted driver cycle is:

1. resolve the current statement from the top cursor
2. interpret it into the top-level machine effect
3. consume that effect through the top-level machine
4. obtain `Shell<I::Machine::Stop>`
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
