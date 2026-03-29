# kirin-interpreter-3 Design

## Motivation

We have experimented with interpreter designs in `kirin-interpreter` and `kirin-interpreter-2`. The trait system in
`kirin-interpreter-2` is roughly the right direction but has several problems we want to address:

1. **Trait surface is too large.** interpreter-2 exposes Machine, ConsumeEffect, Interpreter, Driver, Exec, Invoke, ResolveCallee, Position, Fuel, Breakpoints, Interrupt — too many concepts for dialect authors to parse.

2. **No unified composition algebra.** interpreter-2 has ad-hoc `Lift` and `ProjectMachine`/`ProjectMachineMut` traits. Dialects, machines, effects, and errors each compose differently. We want a single Lift/Project algebra that works uniformly across all of these.

3. **Error handling is fragmented.** interpreter-2 has multiple error types (`InterpreterError`, `ValueStore::Error`, `ConsumeEffect::Error`) with `From` conversion bounds scattered everywhere. We want errors to follow the same Lift algebra as effects.

4. **Frames are over-exposed.** Frame management should be internal to the interpreter, not part of the public trait API.

We are starting a new design from scratch in `kirin-interpreter-3` based on these lessons.

## Design Principles

- **Unified effect type.** A single `Effect<V, Seed, DE>` expresses everything — cursor control, value binding, completion, complex execution (seeds), and dialect machine effects. Both `Interpretable::interpret` and `Machine::consume_effect` use this same type.
- **Lift/Project for composition.** `Lift<Effect<V, S, DEA>> for Effect<V, S, DEC>` handles composed dialects — only the `Machine(de)` variant is transformed. Same pattern for `InterpError<ME>`.
- **Minimal trait surface.** Dialect authors see `Interpreter` (supertrait), `Interpretable`, `Execute` — that's it.
- **Interpreter-specialized semantics.** `Interpretable<I>` is parameterized by the interpreter type because operational semantics depend on the interpreter (concrete vs abstract vs symbolic).
- **Interpreter is a Machine.** `Interpreter: Machine` with `Machine::Effect = Effect<Self::Value, Self::Seed, Self::DialectEffect>`. The interpreter handles all effect variants and delegates `Machine(de)` to the dialect machine.
- **Seeds for complex execution.** When a dialect needs to orchestrate multi-step execution (loops, nested block evaluation), it defines a custom seed type with an `Execute<I>` impl. Seeds consume `self`; dialect operations borrow `&self`.
- **Single-stage concrete first.** Focus on single-stage concrete interpretation. Abstract interpreters express Fork as a machine effect.

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

### Traits
- [traits/](traits/index.md) — trait hierarchy overview
- [traits/lift_and_project.md](traits/lift_and_project.md) — Lift/Project/TryLift/TryProject algebra
- [traits/machine.md](traits/machine.md) — Machine trait, two levels (dialect machines + interpreter)
- [traits/effects.md](traits/effects.md) — Unified `Effect<V, Seed, DE>` type, Lift composition
- [traits/errors.md](traits/errors.md) — `InterpError<ME>`, symmetric with effects
- [traits/interpretable.md](traits/interpretable.md) — Interpretable, Interpreter, sub-traits
- [traits/seed.md](traits/seed.md) — Execute trait, seed types, seed composition

### Implementations
- [interpreter.md](interpreter.md) — Interpreter overview, responsibilities
- [single_stage.md](single_stage.md) — SingleStage concrete interpreter

### Other
- [multi_result.md](multi_result.md) — Product-based multi-result handling
- [examples/](examples/index.md) — Complete API walkthrough with 10 examples

## Comparison with interpreter-2

| Concept | interpreter-2 | interpreter-3 |
|---------|--------------|--------------|
| Composition algebra | Ad-hoc Lift + ProjectMachine | Unified Lift/Project/TryLift/TryProject |
| Trait surface | Machine, ConsumeEffect, Interpreter, Driver, Exec, Invoke, ResolveCallee, Position, Fuel, Breakpoints, Interrupt | Interpreter (Machine + ValueRead + PipelineAccess), Interpretable, Execute |
| Dialect `interpret()` gets | `&mut I` (full interpreter) | `&mut I` (full interpreter) |
| Dialect `interpret()` returns | Dialect-specific effect | `Effect<I::Value, I::Seed, Self::Effect>` (direct construction) |
| Type params on Interpretable | `<'ir, I>` | `<I: Interpreter>` (no lifetime) |
| Effect model | Cursor → Directive (two-level, fixed) | `Effect<V, Seed, DE>` (unified, composable via Lift) |
| Error model | Multiple types, From conversions | `InterpError<ME>` (unified, composable via Lift) |
| Seeds | Fixed Seed types on Machine | Open Execute trait, seeds are reusable |
| Frames | Part of public trait API | Internal to interpreter |

## Deferred

- **Abstract interpreter** — Fork as machine effect, widening/narrowing, fixpoint loop.
- **Multi-stage interpretation** — stage switching, dynamic dialect dispatch across stages.
- **Derive macros** — `#[derive(Interpretable)]` for the new trait. The composition pattern (match + Lift) is mechanical and derivable.
- **Debugging features** — fuel, breakpoints, stepping. Interpreter-specific, addable without changing the trait algebra.
