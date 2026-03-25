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
- optional sibling driver-control traits like `FuelControl`,
  `BreakpointControl`, and `InterruptControl`

It is the preferred shell for:

- dialect-local operational-semantics tests
- programs known to remain within one stage
- fast typed execution without dynamic stage-dispatch overhead

### MVP Checkpoint

The current `kirin-interpreter-2` implementation proves this shell shape in
code, with one intentional narrowing:

- `SingleStageInterpreter` owns `step()`, `run()`, and `run_until_break()` as
  inherent methods for now

The primitive semantic shell trait `Interpreter<'ir>` is implemented and used
by the shell, but the convenience driver APIs have not yet been lifted into
shared provided defaults on the trait itself.

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

## Dynamic Stage Store

`DynamicInterpreter` should be parameterized by a framework `StageStore`
abstraction.

The important storage rule is still the same:

- `StageStore` stores whole stage-local interpreter shells, not raw
  machine/value fragments

This means each stage entry stores a full `SingleStageInterpreter<L>`-like
typed execution unit, including:

- stage-local machine state
- stage-local value store
- stage-local cursor stack
- typed `Interpreter<'ir>` behavior

This avoids reconstructing typed interpreters out of raw parts every time the
dynamic shell resolves `in_stage::<L>()`.

### Family-Relative Storage

The concrete storage direction should be driven by two inputs:

- the stage enum `S`
- the single-stage interpreter family `F`

The stage enum remains the source of truth for which dialect lives at each
`CompileStage`.

The single-stage family decides, for each dialect `L`, which shell and machine
type should be used for that stage in a particular interpretation mode.

This mapping is family-relative, not dialect-absolute.

For example, a concrete-execution family and an abstract-interpretation family
may map the same dialect `L` to different machine and shell types.

The intended shape is:

```rust
trait SingleStageFamily<'ir, S>
where
    S: StageMeta,
{
    type Error;
    type Context;
    type Shell<L>: Interpreter<'ir>
    where
        L: Dialect;

    fn make_context(pipeline: &'ir Pipeline<S>) -> Result<Self::Context, Self::Error>;

    fn build_shell<L>(
        ctx: &Self::Context,
        stage_id: CompileStage,
        stage: &'ir StageInfo<L>,
    ) -> Result<Self::Shell<L>, Self::Error>
    where
        L: Dialect;
}
```

The family context exists because stage-local shell construction may need more
than `&StageInfo<L>` alone. The current interpreter implementations already
depend on pipeline-global state and stage identity in addition to the typed
stage view.

### Derived Store Shape

The default direction should be a pipeline-shaped typed store, not a purely
erased bag of stage shells.

The stage enum `S` should derive a parallel stage-entry layout relative to a
family `F`.

Conceptually:

```rust
trait StageShellLayout<'ir, F>: StageMeta
where
    F: SingleStageFamily<'ir, Self>,
    Self: Sized,
{
    type Entry;
}
```

and the default store becomes:

```rust
struct DerivedStageStore<'ir, S, F>
where
    S: StageMeta + StageShellLayout<'ir, F>,
    F: SingleStageFamily<'ir, S>,
{
    pipeline: &'ir Pipeline<S>,
    family_ctx: F::Context,
    entries: Vec<S::Entry>,
}
```

Each `CompileStage` indexes one entry in `entries`, just like the existing
pipeline and dispatch-cache machinery.

The practical implication is:

- storage is keyed by stage slot, not by dialect type
- duplicate stage variants using the same dialect remain valid
- typed shell access still resolves through `CompileStage` plus `HasStageInfo<L>`

The default derived entry shape should be lazy and stage-local, for example
storing `Option<F::Shell<L>>`-style slots per stage entry:

- `None`
  stage shell has not been initialized yet
- `Some(shell)`
  stage shell is ready

This keeps the public model simple while still allowing lazy initialization and
stage-local preseed overrides.

The framework should provide:

- a `StageStore` trait
- a default derived heterogeneous implementation based on the stage enum plus
  interpreter family
- room for custom user-provided store implementations

`StageStore` should own:

- typed stage-shell lookup
- typed mutable stage-shell lookup
- lazy stage-shell initialization

It should not absorb unrelated policy such as stage-boundary adapter
resolution.

An erased framework store may still be useful later, but it should be treated
as an implementation strategy layered on top of this typed stage-layout model,
not as the primary design center.

## Dynamic Orchestration Stack

Each stored stage-local shell owns its own cursor stack.

Same-stage nesting remains entirely inside that stage-local shell.

`DynamicInterpreter` should still own a separate lightweight orchestration
stack, but only for cross-stage resumption.

One orchestration frame should conceptually carry:

- caller stage identity
- target stage identity
- boundary adapter resume payload

This stack is not:

- a dialect semantic call stack
- a replacement for stage-local cursor stacks

It is only the cross-stage continuation stack used to resume the caller-side
boundary protocol after target-stage execution returns.

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

The current MVP implementation narrows this slightly:

- `SingleStageInterpreter` provides these driver methods directly
- `run_until_break()` is currently the same implementation as `run()`, because
  the single-stage shell already stops on any suspension reason

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

Cross-stage driver state should remain separate from stage-local execution
state:

- stage-local shells own same-stage execution and cursor progression
- the dynamic shell owns only stage selection and cross-stage orchestration

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

## Deferred Storage Decisions

The following storage questions are intentionally left open for the MVP
implementation phase:

- the exact public `StageStore` trait surface
- whether callback-style stage access helpers are needed in v1, or can be
  added later over direct borrowed access
- the exact derive name and crate placement for `StageShellLayout`-style code
  generation
- the public API for preseeded stage-shell overrides
- whether the framework should ship an erased store wrapper in the first
  dynamic-shell implementation

These should be revisited only after the single-stage concrete interpreter MVP
has stabilized the typed shell and machine mechanism.

For `DynamicInterpreter`, these control traits operate on shared shell state:

- one shared breakpoint set keyed by stage and execution location
- one shared fuel counter
- one shared latched host-interrupt flag

Typed stage views into the dynamic shell should forward into that shared
driver-control state rather than owning their own independent breakpoint or
fuel storage.

The shell-policy split should be explicit:

- low-level typed semantic APIs ignore breakpoint, fuel, and interrupt policy
- driver APIs (`step`, `run`, `run_until_break`) apply that suspension policy

The current single-stage shell also keeps a small internal post-step
checkpoint so `ExecutionLocation::AfterStatement(_)` remains observable for
breakpoints without widening the public machine semantics API.
