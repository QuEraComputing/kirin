# Interpreter Shells

## Two Concrete Shells

The framework should support two concrete shells sharing one typed shell
contract:

- `SingleStageInterpreter<L>`
- `DynamicInterpreter`

The goal is to let dialect operational semantics remain portable while still
supporting both:

- fast, typed, stage-local execution
- staged programs with cross-stage switching

## `SingleStageInterpreter<L>`

`SingleStageInterpreter<L>` is the typed execution shell for one language at
one stage.

It should own one top-level machine for that stage along with:

- one typed value store
- one top-level machine
- one cursor stack
- one semantic stop payload type through `Machine::Stop`

It should implement the typed shell traits directly:

- `Interpreter<'ir>`
- `ValueStore`
- `StageAccess<'ir>`
- typed effect inspection and consumption APIs
- driver APIs such as `step`, `run`, and `run_until_break`
- optional sibling driver-control traits like `FuelControl` and
  `BreakpointControl`

It is the preferred shell for:

- dialect-local operational-semantics tests
- programs known to remain within one stage
- fast typed execution without dynamic stage-dispatch overhead

## `DynamicInterpreter`

`DynamicInterpreter` is the stage-dynamic orchestrator.

It should be modeled as a heterogeneous collection of stage-local
single-stage interpreters rather than as one monolithic interpreter with one
global `Value`, `Machine`, and `Effect` type.

Each stage entry may have its own:

- value type
- machine type
- effect type
- stop payload type

The dynamic shell owns:

- the active stage selection
- driver loops
- breakpoint/fuel policy across staged execution
- stage switching
- stage-boundary orchestration

The dynamic shell is not itself the typed `Interpreter<'ir>` surface.
Typed APIs remain on:

- `SingleStageInterpreter<L>`
- typed stage-specific views returned from `in_stage::<L>()`

These typed stage views should implement the same `Interpreter<'ir>` contract
as the single-stage shell, so dialect operational semantics can stay portable
across both execution modes.

## Dynamic And Typed APIs

The direct interpreter API should be stage-dynamic.

The `in_stage::<L>()` API is for stage-specific typed access.

This yields the public split:

- stage-dynamic shell APIs
  - `step()`
  - `run()`
  - `run_until_break()`
- typed stage-specific APIs
  - `interpret_current()`
  - `interpret_local(stmt)`
  - `interpret_lifted(stmt)`
  - `consume_local_effect(effect)`
  - `consume_lifted_effect(effect)`
  - `consume_effect(effect)`
  - `consume_local_control(control)`
  - `consume_control(control)`
  - `step()`

The typed effect/value APIs belong on `SingleStageInterpreter<L>` and on typed
stage handles, not on the dynamic shell.

## Typed Step APIs

The typed stepping surface should support both low-level and convenience forms.

Low-level:

- `interpret_current()`
- `interpret_local(stmt)`
- `interpret_lifted(stmt)`
- `consume_local_effect(effect)`
- `consume_lifted_effect(effect)`
- `consume_effect(effect)`
- `consume_local_control(control)`
- `consume_control(control)`

Convenience:

- `step()`

`step()` should return `StepOutcome`:

- `Stepped(StepResult { effect, control })`
- `Suspended(SuspendReason)`
- `Completed`

This keeps typed execution useful for fine-grained testing and debugging while
still behaving like a driver API.

The default implementation story should be:

- `step()` is a conditional provided default when the returned effect/control
  artifacts are cloneable
- `run()` and `run_until_break()` are provided defaults that loop directly over
  the shell primitives and do not inherit `step()` clone bounds

## Dynamic Driver APIs

The dynamic shell should keep the familiar high-level driver operations:

- `step()`
- `run()`
- `run_until_break()`

These APIs are intentionally semantic-effect opaque. They are for execution
control, not effect inspection.

`step()` on the dynamic shell should still follow the same driver policy as the
typed shell:

- breakpoint and interrupt checks happen before executing the current statement
- fuel is decremented only when a statement is actually executed
- if the final statement runs, that call still counts as a step rather than a
  `Completed` no-op

## Stage-Switch Behavior

Both shells should expose the same public stage-switch capability on their typed
stage-specific views.

Behavior differs by shell:

- `SingleStageInterpreter<L>`
  returns a defined runtime error when a statement requests stage switching
- `DynamicInterpreter`
  executes the switch through the stage-boundary protocol

This lets the same dialect semantics run on both shells:

- same-stage semantics can be tested in `SingleStageInterpreter<L>`
- cross-stage semantics can be exercised in `DynamicInterpreter`

## Driver Control Traits

Fuel and breakpoints should remain separate sibling traits rather than
supertraits of `Interpreter<'ir>`.

The intended layering is:

- `Interpreter<'ir>`
  typed semantic shell
- `FuelControl`
  shell fuel policy
- `BreakpointControl`
  shell breakpoint management
- `InterruptControl`
  shell host-interrupt policy

This keeps the main typed shell trait focused while still exposing the driver
controls on concrete shells and typed views that support them.

These controls are shell state, not semantic machine state:

- they do not belong in `Machine<'ir>` composition
- they should not be projected through `ProjectMachine`

For `DynamicInterpreter`, both traits operate on shared shell state:

- one shared breakpoint set keyed by stage and execution location
- one shared fuel counter
- one shared latched host-interrupt flag

Typed stage views into the dynamic shell should forward into that shared
driver-control state rather than owning their own independent breakpoint or
fuel storage.

The shell-policy split should be explicit:

- low-level typed semantic APIs ignore breakpoint, fuel, and interrupt policy
- driver APIs (`step`, `run`, `run_until_break`) apply that suspension policy
