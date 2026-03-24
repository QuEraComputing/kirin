# Interpreter Shells

## Two Concrete Shells

The framework should support two concrete interpreter shells sharing one kernel
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

It should own:

- one value store for `L`
- one root semantic state for `L`
- one cursor stack
- one semantic stop payload type

It should expose:

- `Machine<'ir>`
- typed `ValueStore`
- typed effect inspection APIs
- `state()` and `state_mut()`
- stage-specific execution helpers

It is the preferred shell for:

- dialect-local operational-semantics tests
- programs known to remain within one stage
- fast typed execution without dynamic stage-dispatch overhead

## `DynamicInterpreter`

`DynamicInterpreter` is the stage-dynamic orchestrator.

It should be modeled as a heterogeneous collection of stage-local
single-stage interpreters rather than as one monolithic interpreter with one
global `Value`, `State`, and `Effect` type.

Each stage entry may have its own:

- value type
- state type
- effect type
- stop payload type

The dynamic shell owns:

- the active stage selection
- driver loops
- breakpoint/fuel policy across staged execution
- stage switching
- stage-boundary orchestration

The dynamic shell should not expose raw typed value/effect APIs directly.
Typed APIs remain on stage-specific views or single-stage interpreters.

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
  - `consume_effect(effect)`
  - `apply_action(action)`
  - `consume_and_apply(effect)`
  - `step()`

The typed effect/value APIs belong on `SingleStageInterpreter<L>` and on typed
stage handles, not on the dynamic shell.

## Typed Step APIs

The typed stepping surface should support both low-level and convenience forms.

Low-level:

- `interpret_current()`
- `consume_effect(effect)`
- `apply_action(action)`
- `consume_and_apply(effect)`

Convenience:

- `step()`

`step()` should return a step-result artifact carrying:

- the full language effect value
- the applied `KernelAction`

This keeps typed execution useful for fine-grained testing and debugging.

## Dynamic Driver APIs

The dynamic shell should keep the familiar high-level driver operations:

- `step()`
- `run()`
- `run_until_break()`

These APIs are intentionally semantic-effect opaque. They are for execution
control, not effect inspection.

## Stage-Switch Behavior

Both shells should implement the same public stage-switch capability.

Behavior differs by shell:

- `SingleStageInterpreter<L>`
  returns a defined runtime error when a statement requests stage switching
- `DynamicInterpreter`
  executes the switch through the stage-boundary protocol

This lets the same dialect semantics run on both shells:

- same-stage semantics can be tested in `SingleStageInterpreter<L>`
- cross-stage semantics can be exercised in `DynamicInterpreter`
