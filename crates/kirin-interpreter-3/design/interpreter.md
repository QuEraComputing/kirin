# Interpreter

The interpreter is a special Machine that doesn't belong to any dialect. It is the shell for the
entire IR — parameterized by a dialect machine, it handles everything the IR hard-codes:

**Interpreter responsibilities:**
- ValueStore: managing SSA values and their bindings
- Call stack / frames: push, pop, continuation management
- IR cursor: tracking the current statement position
- Execution loop: when to advance, stop, etc.

**NOT interpreter responsibilities:**
- Dialect-specific semantics (that's `Interpretable`)
- How to execute Block, Region, UnGraph, DiGraph (that's `Execute` on seed types)

Frames, cursors, and the call stack are **internal** — not part of the public trait API.

## Trait Design

See [traits/](traits/index.md) for the full trait surface:

- [Machine](traits/machine.md) — `consume_effect`, effect/error associated types
- [Interpreter](traits/interpretable.md#interpreter-supertrait) — `step`, `run`, associated types (Seed, DialectEffect, DialectError)
- [Interpretable](traits/interpretable.md) — dialect semantics (`&self`)
- [Execute](traits/seed.md) — seed execution (`self`)
- [Effect](traits/effects.md) — unified `Effect<V, Seed, DE>` type
- [Lift/Project](traits/lift_and_project.md) — composition algebra

## Implementations

- [SingleStage](single_stage.md) — single-stage concrete interpreter (**initial focus**)

### Deferred Implementations

- **Multi-stage concrete** — dynamic dispatch on stages, language switching
- **Abstract** — fixpoint execution, widening/narrowing (Fork as machine effect)
