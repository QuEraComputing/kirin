# Kirin Interpreter-4 Design

**Date:** 2026-04-01 (updated)
**Status:** execution seeds implemented, 10 tests passing
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
- user-definable cursor types with `Execute<I>` trait
- global cursor stack with flat driver loop (zero Rust recursion)
- `Action<V, R, C>` effect algebra with Return/Yield/Push/Call
- mixed-flavor effects (mutation + return effects in the same `interpret`)
- unified `Lift`/`Project` for machines, effects, cursors, and errors

## Key Decisions

1. `Interpretable::interpret(&self, interp: &mut I) -> Result<Effect, Error>`
   gives dialect authors full mutation access AND an effect return channel.

2. **Execution seeds are cursor types**, not interpreter methods. Each cursor
   implements `Execute<I>` with `&mut self`. The initial construction encodes
   the starting point; mutation tracks traversal progress. There is no separate
   "seed" concept.

3. **Global cursor stack** — `Vec<C>` on the interpreter, not per-frame. The
   driver pops, calls `execute`, dispatches the returned `Action`. All nesting
   is on the cursor stack with zero Rust recursion.

4. **Call/Return as driver-handled effects.** The frame stack is
   interpreter-internal state. Cursors return `Call` effects; the driver pushes
   frames. This eliminates `FunctionCursor` for standard function calls.

5. The interpreter IS a `Machine`. Its `consume_effect` only handles `Delegate`
   (inner machine effects). All structural effects (Push, Yield, Return, Call)
   are handled by the driver loop.

6. Stage dispatch uses the pipeline's `StageInfo` directly. Single-stage
   interpreters have `StageInfo<L>`, multi-stage have a stage enum with
   `HasStageInfo<L>` dispatch.

7. `Receipt` trait for type parameter bundling is deferred until patterns
   stabilize.

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

Interpreter-4 replaces the shell's hardcoded traversal with user-definable
cursor types. Dialect authors return `Push(BlockCursor)` to execute inline
bodies (like `scf.if` branches) and the flat driver loop handles all nesting.
The key win: `scf.for` can use `ForCursor` on the cursor stack — each iteration
is one driver cycle, with zero Rust recursion regardless of nesting depth.

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
