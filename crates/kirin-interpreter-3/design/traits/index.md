# Trait Design

This directory defines the public trait surface of `kirin-interpreter-3`.

## Trait Hierarchy

```text
Machine                     — stateful effect consumer
├── ValueRead               — SSA value access
├── PipelineAccess          — pipeline/stage/callee resolution
└── Interpreter             — Machine + ValueRead + PipelineAccess + execution loop

Interpretable<I>            — per-statement dialect semantics
Execute<I>                  — multi-step control orchestration

Lift/Project/TryLift/TryProject — sum/product composition algebra
```

## Design Notes

- The trait surface is intentionally small.
- There is no public `ProjectMut` escape hatch for dialect semantics.
- Stateful dialects communicate through `Effect::Machine(de)`.
- Seeds are separate from the effect algebra and are documented in
  [seed.md](seed.md).

## Documents

- [lift_and_project.md](lift_and_project.md) — Lift/Project/TryLift/TryProject algebra
- [machine.md](machine.md) — `Machine` trait, dialect machines, interpreter shell
- [effects.md](effects.md) — `Effect<V, DE>` and effect composition
- [errors.md](errors.md) — `InterpError<DE>` and error composition
- [interpretable.md](interpretable.md) — `Interpretable`, `Interpreter`, `ValueRead`, `PipelineAccess`
- [seed.md](seed.md) — `Execute`, reusable sub-execution executors, and seed invariants
