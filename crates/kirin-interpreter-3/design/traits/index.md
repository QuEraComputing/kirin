# Trait Design

This directory defines the public trait surface of kirin-interpreter-3.

## Trait Hierarchy

```
Machine                     — stateful effect consumer
├── ValueRead               — SSA value access
├── PipelineAccess          — pipeline/stage/callee resolution
└── Interpreter             — Machine + ValueRead + PipelineAccess + execution

Interpretable<I>            — dialect semantics (borrows &self)
Execute<I>                  — seed execution (consumes self)

Lift/Project/TryLift/TryProject — composition algebra
ProjectMut<T>               — mutable machine projection
```

## Documents

- [lift_and_project.md](lift_and_project.md) — Lift/Project/TryLift/TryProject algebra, ProjectMut
- [machine.md](machine.md) — Machine trait, two levels (dialect machines + interpreter), composition
- [effects.md](effects.md) — Unified `Effect<V, Seed, DE>` type, Lift composition
- [errors.md](errors.md) — `InterpError<ME>`, symmetric with effects
- [interpretable.md](interpretable.md) — Interpretable trait, Interpreter supertrait, sub-traits
- [seed.md](seed.md) — Execute trait, seed types, seed composition
