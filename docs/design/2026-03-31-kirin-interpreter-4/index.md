# Kirin Interpreter-4 Design

**Date:** 2026-03-31
**Status:** design iteration, MVP implemented
**Primary crate:** `crates/kirin-interpreter-4`

## Summary

Interpreter-4 synthesizes the strengths of the three previous interpreter
iterations while fixing the key problem: the interpreter-3 design hardcoded IR
traversal into the shell, removing dialect authors' ability to control how they
execute the bodies their statements contain.

The core changes from interpreter-3:

- **execution seeds are callable by dialect authors**, not just consumed by the
  shell. A dialect author implementing `scf.for` can call `interp.exec_block()`
  in a loop. A dialect author implementing `cf.branch` can return a `Jump`
  effect for the shell to handle. Both paths coexist.
- **mixed-flavor effect system**: mutation via `&mut I` for local state changes,
  return effects for cross-cutting concerns. The dialect author chooses.
- **stage dispatch returns to the interpreter** (like kirin-interpreter v1),
  using the pipeline's stage info directly.

## What Carries Forward

From **kirin-interpreter** (v1):
- stage dispatch inside the interpreter via pipeline + `HasStageInfo<L>`
- callee resolution query builder
- trait decomposition (`ValueStore`, `PipelineAccess`)
- `Interpretable` and `Interpreter` as the primary dialect and shell contracts

From **kirin-interpreter-2** (interpreter-3 design):
- cursor mechanism decoupled from call stack
- frame and frame stack abstraction
- execution seeds (Block, Region, DiGraph, UnGraph)
- `Shell` control language for cursor directives
- driver/position/control trait layering

From **kirin-interpreter-3** (machine design):
- everything is a Machine
- lift/project for uniform type conversion
- dialect/machine/effect/error composition rules
  (dialect = sum, machine = product, effect = sum, error = sum)
- `Interpretable` as mutation over machine projection + returned effects

New in **interpreter-4**:
- execution seeds as interpreter methods callable during `interpret`
- mixed-flavor effects (mutation + return effects in the same `interpret`)
- `Receipt` trait for type parameter bundling
- unified `Lift`/`Project` for machines, effects, and errors

## Key Decisions

1. `Interpretable::interpret(&self, interp: &mut I) -> Result<Effect, Error>`
   gives dialect authors full mutation access AND an effect return channel.

2. Execution seeds (`exec_block`, `exec_region`, `invoke`) are **methods on the
   interpreter**, not effect types. Dialect authors call them synchronously.

3. Effects are for **deferred control flow** — cursor changes, frame operations,
   and stop signals that the shell handles after `interpret` returns.

4. The interpreter IS a `Machine`. Its `consume_effect` dispatches to inner
   dialect machines and handles cursor/frame effects.

5. Stage dispatch uses the pipeline's `StageInfo` directly. Single-stage
   interpreters have `StageInfo<L>`, multi-stage have a stage enum with
   `HasStageInfo<L>` dispatch.

6. `Receipt` bundles all type parameters (Language, Value, Machine, StageInfo,
   Error) to keep concrete interpreter signatures manageable.

## Document Map

- [machines-and-effects.md](machines-and-effects.md)
  Machine trait, effect composition, lift/project, mixed-flavor effects
- [execution-seeds.md](execution-seeds.md)
  Inline execution seeds, body runners, callee resolution
- [concrete-interpreter.md](concrete-interpreter.md)
  ConcreteInterpreter shape, Receipt trait, driver loop, stage dispatch
- [mvp-report.md](mvp-report.md)
  Implementation status, decisions with rationale, and deferred work

## Relation To Previous Designs

### vs interpreter-3 (machine design)

Interpreter-3 forced all body execution through the `Shell` control language
(`Push`/`Replace`/`Pop`). This meant:
- `scf.for` couldn't loop its body inline — it had to return a `Push` effect
  and rely on the shell to loop
- the shell's `inherit` method hardcoded traversal strategies
- dialect authors lost the power to compose execution patterns

Interpreter-4 keeps the `Shell` control language for cursor-level effects
(Jump, Stop) but adds inline execution seeds that dialect authors call during
`interpret`. The `Shell` enum becomes the **deferred** path; inline seeds are
the **synchronous** path.

### vs kirin-interpreter (v1)

Interpreter v1 coupled execution logic tightly to `BlockEvaluator::eval_block`
and `CallSemantics::eval_call`. These were the right ideas but the trait surface
was too coarse — every interpreter type had to implement the full eval_block
loop.

Interpreter-4 extracts the execution strategies into composable seed methods
that the interpreter provides and dialect authors consume. The loop logic moves
from trait methods to interpreter implementations.

### vs kirin-interpreter-2 (refactor design)

The refactor design centered around body-shape executors (`ExecBlock`,
`ExecRegion`). Interpreter-4 reuses this framing but places the execution seeds
on the interpreter trait surface rather than as standalone function traits.
