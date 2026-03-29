# kirin-interpreter-3 Design

## Motivation

We have experimented with interpreter designs in `kirin-interpreter` and `kirin-interpreter-2`. The trait system in
`kirin-interpreter-2` is roughly the right direction but has several problems we want to address:

1. **Trait surface is too large.** interpreter-2 exposes Machine, ConsumeEffect, Interpreter, Driver, Exec, Invoke, ResolveCallee, Position, Fuel, Breakpoints, Interrupt — too many concepts for dialect authors to parse.

2. **No unified composition algebra.** interpreter-2 has ad-hoc `Lift` and `ProjectMachine`/`ProjectMachineMut` traits. Dialects, machines, effects, and errors each compose differently. We want a single Lift/Project algebra that works uniformly across all of these.

3. **Error handling is fragmented.** interpreter-2 has multiple error types (`InterpreterError`, `ValueStore::Error`, `ConsumeEffect::Error`) with `From` conversion bounds scattered everywhere. We want errors to follow the same Lift/Project algebra as effects.

4. **Frames are over-exposed.** Frame management should be internal to the interpreter, not part of the public trait API.

We are starting a new design from scratch in `kirin-interpreter-3` based on these lessons.

## Design Principles

- **Lift/Project everywhere.** One algebra for composing dialects, machines, effects, and errors. Infallible (`Lift/Project`) and fallible (`TryLift/TryProject`) variants mirror `From/Into` and `TryFrom/TryInto`.
- **Minimal trait surface.** Dialect authors see `Interpreter` (supertrait), `Interpretable`, `Execute` — that's it.
- **Interpreter-specialized semantics.** `Interpretable<I>` is parameterized by the interpreter type because operational semantics depend on the interpreter (concrete vs abstract vs symbolic).
- **GAT-based effect/error types.** `I::Effect<DE>` and `I::Error<ME>` are GATs parameterized by the dialect's own machine effect/error types. Uniform `try_lift()` for all conversions — no `Ok()` wrappers needed.
- **Layered effects.** Base effects (shared by all interpreters) live in a `Base(...)` slot. Dialect machine effects live in a `Machine(DE)` slot. Interpreter-specific effects (Execute, Fork) are internal.
- **Seeds for complex execution.** When a dialect needs to orchestrate multi-step execution (loops, nested block evaluation), it defines a custom seed type with an `Execute<I>` impl.
- **Single-stage concrete first.** Focus on single-stage concrete interpretation, but leave room for abstract interpretation.

## Running Example

The design documents use the following example dialects:

```rust
#[derive(Dialect)]
pub enum DialectA { OpA(..), OpB(..) }

#[derive(Dialect)]
pub enum DialectB { OpA(..), OpB(..) }

#[derive(Dialect)]
pub enum DialectC { A(DialectA), B(DialectB) }
```

With corresponding machines:

```rust
struct MachineA { /* DialectA's state */ }
struct MachineB { /* DialectB's state */ }
struct MachineC { a: MachineA, b: MachineB }  // product composition
```

## Document Index

- [lift_and_project.md](lift_and_project.md) — Lift/Project/TryLift/TryProject algebra
- [machine.md](machine.md) — Machine trait, effect consumption, dialect machines
- [effects.md](effects.md) — Layered effect types (BaseEffect, interpreter GAT, machine effects)
- [errors.md](errors.md) — Error model (symmetric with effects, GAT-based)
- [interpretable.md](interpretable.md) — Interpretable trait, Interpreter supertrait
- [seed.md](seed.md) — Seed & Execute pattern for complex execution
- [interpreter.md](interpreter.md) — Interpreter as a machine, SingleStage, execution loop
- [multi_result.md](multi_result.md) — Product-based multi-result handling
- [examples/](examples/index.md) — Complete API walkthrough with 10 examples

## Comparison with interpreter-2

| Concept | interpreter-2 | interpreter-3 |
|---------|--------------|--------------|
| Composition algebra | Ad-hoc Lift + ProjectMachine | Unified Lift/Project/TryLift/TryProject |
| Trait surface | Machine, ConsumeEffect, Interpreter, Driver, Exec, Invoke, ResolveCallee, Position, Fuel, Breakpoints, Interrupt | Interpreter (Machine + ValueRead + PipelineAccess), Interpretable, Execute |
| Dialect `interpret()` gets | `&mut I` (full interpreter) | `&mut I` (full interpreter, effect return as discipline) |
| Dialect `interpret()` returns | Dialect-specific effect | `I::Effect<Self::Effect>` (GAT, uniform try_lift) |
| Type params on Interpretable | `<'ir, I>` | `<I: Interpreter>` (no lifetime) |
| Effect model | Cursor -> Directive (two-level, fixed) | BaseEffect + GAT (layered, composable via Seq + Lift between GAT instantiations) |
| Error model | Multiple types, From conversions | Same GAT algebra as effects, uniform try_lift |
| Seeds | Fixed Seed types on Machine | Open Execute trait, seeds are reusable |
| Frames | Part of public trait API | Internal to interpreter |

## Deferred

- **Abstract interpreter** — Fork effect, widening/narrowing, fixpoint loop. The GAT effect model leaves room.
- **Multi-stage interpretation** — stage switching, dynamic dialect dispatch across stages.
- **Derive macros** — `#[derive(Interpretable)]` for the new trait. The composition pattern (match + try_lift) is mechanical and derivable.
- **Builder patterns** — ergonomic effect construction helpers.
- **Debugging features** — fuel, breakpoints, stepping. Interpreter-specific, addable without changing the trait algebra.
