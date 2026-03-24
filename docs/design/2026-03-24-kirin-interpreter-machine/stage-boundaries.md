# Stage Boundaries

## Stage Switching Is A Semantic Action

Stage switching is not shell control like `Push` or `Pop`.

It is a semantic action initiated by dialect code through a public interpreter
capability.

Two common sources are:

- a call convention that contains an abstract `Function` plus a target-stage
  symbol
- a call convention that contains a `StagedFunction` naming another stage

Dialect authors implement these conventions in `Interpretable` for the current
stage language. They use a public stage-switch API rather than manipulating
shell internals directly.

## Shared Capability, Different Shell Behavior

The stage-switch capability should be shared by both shells through their typed
stage-specific interpreter views:

- `SingleStageInterpreter<L>`
  reports a defined runtime error such as missing target-stage context or stage
  switch unsupported
- `DynamicInterpreter`
  executes the switch correctly through the stage-boundary protocol

This is deliberate. The same dialect semantics can be exercised:

- in same-stage form with a single-stage interpreter
- in cross-stage form with the dynamic interpreter

## Boundary Protocol

Stage switching should use a hybrid design:

- stage-pair or language-pair traits define conversions and boundary behavior
- the dynamic interpreter orchestrates the switch

This boundary protocol should be more structured than plain `Into` or `From`
because cross-stage switching may be:

- fallible
- stage-pair specific
- metadata dependent
- more than just value conversion

It should leave room for abstract-interpretation boundaries later.

The high-level boundary protocol should be a neutral stage-pair adapter, not a
caller-owned or callee-owned trait.

Its public contract should be caller-facing:

```rust
enum StageBoundaryResult<R, S> {
    Returned(R),
    Trapped(S),
    Suspended(SuspendReason),
    Completed,
}
```

The boundary adapter should own its own protocol-specific payload types:

- `Input`
- `Return`
- `Stop`
- `Error`

The intended high-level shape is:

```rust
trait StageBoundary<'ir, From, To> {
    type Input;
    type Return;
    type Stop;
    type Error;

    fn execute<CF, CT>(
        &self,
        caller: &mut CF,
        caller_stage: &'ir StageInfo<From>,
        target: &mut CT,
        target_stage: &'ir StageInfo<To>,
        seed: ExecutionSeed,
        input: Self::Input,
    ) -> Result<StageBoundaryResult<Self::Return, Self::Stop>, Self::Error>
    where
        CF: Interpreter<'ir>,
        CT: Interpreter<'ir>;
}
```

Important properties of this shape:

- `From` and `To` are explicit in the trait identity
- the trait is implemented by standalone boundary adapter values
- the trait is stateful/configurable because boundary adapters are value objects
- the adapter receives both typed interpreter views and both typed stage-info
  values
- no public intermediate target-stage run result is exposed
- returned values and trapped/stop payloads are already translated into the
  caller-facing boundary world

`Completed` remains in the boundary result because some boundary protocols may
accept completion without explicit return, while others may treat it as an
error. That policy belongs to the boundary adapter.

## General And Callable-Specific Entry

The public stage-switch layer should provide both:

- a general "execute seed in stage X" entry
- a callable-specific helper built on top of that general entry

The general entry is important because call is not the only meaningful
cross-stage operation.

At the dynamic-shell surface, this should become two APIs:

- `execute_in_stage(target_stage, seed, input)`
- `execute_in_stage_with(boundary, target_stage, seed, input)`

The first path resolves the boundary adapter internally from stage metadata or
boundary registration. The second path takes an explicit boundary adapter value
and is better for tests and advanced control.

On typed caller-stage views, `From` should be inferred from the view and only
`To` remains explicit:

- `execute_in_stage::<To>(target_stage, seed, input)`
- `execute_in_stage_with::<To>(boundary, target_stage, seed, input)`

The target stage should support both forms:

- precise typed target: `&'ir StageInfo<To>`
- convenience runtime target: `CompileStage`

The typed form is the exact contract. The runtime form is an ergonomic helper
that resolves and validates `To` before boundary execution starts.

## Dynamic Stage Storage

`DynamicInterpreter` should own a stage-indexed heterogeneous store of
single-stage interpreters.

Each stage entry may be initialized:

- eagerly by user-provided state
- lazily by a stage-specific factory
- or by a hybrid mix of both

This avoids allocating state for stages that are never executed while still
allowing deterministic handcrafted test setup.

Boundary adapters should operate on typed stage views, not on the dynamic shell
directly. This keeps:

- caller/target machine access explicit
- value conversion statically typed
- boundary logic testable independently from dynamic shell orchestration

## Same-Stage Rule

Stage-boundary execution is only for cross-stage execution.

If a dialect resolves a target back to the current stage, it should use the
ordinary same-stage nested execution helpers instead of going through the
boundary protocol.

The policy should be:

- typed helpers short-circuit same-stage requests to the normal same-stage path
- the dynamic shell still validates that boundary execution is truly cross-stage
- if a same-stage request reaches the dynamic boundary layer, it is a shell
  error

## Host-Driven Switching

The dynamic shell should support both:

- semantic stage switches requested by dialect code
- host-driven stage switching for tests, debugging, and tooling

Normal execution should be driven by semantic stage-switch effects and helpers,
but host-driven switching is still useful and should remain supported.
