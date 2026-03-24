# Testing And Rollout

## Core Runtime Tests In `kirin-interpreter-2`

- recursive calls use the explicit frame stack
- pending consumer resumes the correct statement after `Return`
- yielded values resume the correct parent boundary
- breakpoint stop and resume works through `ExecutionLocation`
- `BlockCursor`, `RegionCursor`, `DiGraphCursor`, and `UnGraphCursor` follow
  correct transitions

## Dialect-Facing Integration Tests

Use small test dialects to prove that the same runtime supports multiple result
conventions:

- strict single-result convention
- implicit tuple/product convention using a dialect-owned `Tuple(Product<Self>)`
  value variant
- `scf.if` and `scf.for`-style nested execution using `ConsumeResult`

## Graph-Boundary Tests

Once graph visitation lands:

- a compound graph node consumes nested execution results through
  `ConsumeResult`
- a small toy language can define a computational-graph statement with a
  `DiGraph` body
- its outward result should match a reference execution of the equivalent
  computation through plain block execution
- this comparison is output-level only and does not require implementing a
  second block-based version of the same language
- breakpoint locations remain statement-based inside graph execution

## Recommended Initial Scope

The first implementation of `kirin-interpreter-2` should include:

- the core trait family
- stage-dynamic dispatch cache and per-frame dispatch entries
- the typed `Staged<'a, ...>` facade
- the internal cursor model
- reusable frame and frame-stack infrastructure
- the concrete stack-based runtime
- interpreter-global state support
- fuel, max-depth, breakpoint, and halt control surfaces
- block and region execution
- callable-body abstraction with a blanket CFG-region path
- explicit call-stack handling through `ExecEffect::Call`
- statement-owned `ConsumeResult`

Graph visitation traits should be designed in this crate from the start, but
their first concrete execution behavior can land after block and region
execution is stable.

Before downstream dialect migration begins, the workspace should also have:

- a separate `kirin-derive-interpreter-2` crate implementing the approved
  derive surface for the new runtime
- shared forwarding and body-field helpers in `kirin-derive-toolkit` that the
  new derive crate is built on

## Follow-Up Planning

Implementation planning should focus on:

1. core runtime data structures and traits
2. explicit frame and pending-consumer state
3. block and region stepping
4. concrete call and return handling
5. graph visitation surfaces without overcommitting to graph scheduling
6. new shared forwarding and body templates in `kirin-derive-toolkit`
7. a separate `kirin-derive-interpreter-2` package
8. migrating one or two representative dialects only after the new derive
   package is finished
