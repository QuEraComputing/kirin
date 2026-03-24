# Stage Boundaries

## Stage Switching Is A Semantic Action

Stage switching is not a kernel primitive like `Push` or `Pop`.

It is a semantic action initiated by dialect code through a public interpreter
capability.

Two common sources are:

- a call convention that contains an abstract `Function` plus a target-stage
  symbol
- a call convention that contains a `StagedFunction` naming another stage

Dialect authors implement these conventions in `Interpretable<L>` for the
current stage language. They use a public stage-switch API rather than
manipulating kernel internals directly.

## Shared Capability, Different Shell Behavior

The stage-switch capability should be shared by both shells:

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

## General And Callable-Specific Entry

The public stage-switch layer should provide both:

- a general "execute seed in stage X" entry
- a callable-specific helper built on top of that general entry

The general entry is important because call is not the only meaningful
cross-stage operation.

## Dynamic Stage Storage

`DynamicInterpreter` should own a stage-indexed heterogeneous store of
single-stage interpreters.

Each stage entry may be initialized:

- eagerly by user-provided state
- lazily by a stage-specific factory
- or by a hybrid mix of both

This avoids allocating state for stages that are never executed while still
allowing deterministic handcrafted test setup.

## Host-Driven Switching

The dynamic shell should support both:

- semantic stage switches requested by dialect code
- host-driven stage switching for tests, debugging, and tooling

Normal execution should be driven by semantic stage-switch effects and helpers,
but host-driven switching is still useful and should remain supported.
