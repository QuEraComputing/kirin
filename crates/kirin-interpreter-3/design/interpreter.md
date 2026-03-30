# Interpreter

The interpreter is a special `Machine` that forms the shell around the IR. It does not define
dialect semantics; it owns execution mechanics.

## Interpreter Responsibilities

- SSA value storage and lookup
- Cursor tracking
- Block dispatch and jump handling
- Frame stack and call/return mechanics
- The step/run execution loop
- Routing `Effect::Machine(de)` into the dialect machine

## Not Interpreter Responsibilities

- Per-statement semantics for dialect operations
- Dialect-local state transitions beyond consuming `Machine(de)`
- Ad-hoc custom control APIs for every dialect

## Key Boundary

The interpreter owns the execution shell. Dialects return effects and invoke seeds;
the interpreter decides how those effects mutate execution state.

This boundary is why:

- frames and cursors stay internal
- dialect semantics stay inside `Interpretable`
- seeds orchestrate control flow without becoming effect variants

## Trait Design

See [traits/](traits/index.md) for the full public surface:

- [Machine](traits/machine.md) — effect consumers
- [Interpreter](traits/interpretable.md#interpreter-supertrait) — step/run shell
- [Interpretable](traits/interpretable.md) — statement semantics
- [Execute](traits/seed.md) — multi-step control orchestration
- [Effect](traits/effects.md) — observable state transitions
- [Lift/Project](traits/lift_and_project.md) — composition algebra

## Implementations

- [SingleStage](single_stage.md) — the first concrete interpreter

## Deferred Implementations

- Multi-stage concrete interpreter
- Abstract interpreter with fork/worklist semantics
