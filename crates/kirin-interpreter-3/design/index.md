# kirin-interpreter-3 Design

`kirin-interpreter-3` is a fresh interpreter experiment built from the lessons of
`kirin-interpreter` and `kirin-interpreter-2`. This document is the normative contract
for the new design.

## References

These references informed the design choices in this note:

- [MLIR Interfaces](https://mlir.llvm.org/docs/Interfaces/) — narrow per-operation semantic
  interfaces are preferable to a large monolithic interpreter API.
- [xDSL interpreter.py](https://github.com/xdslproject/xdsl/blob/main/xdsl/interpreter.py) —
  block execution should return explicit terminal information rather than exposing frame
  internals to operations.
- [rustc const-eval machine.rs](https://github.com/rust-lang/rust/blob/master/compiler/rustc_const_eval/src/interpret/machine.rs)
  — the interpreter shell should own stack, cursor, and call mechanics.
- [Cranelift step.rs](https://github.com/bytecodealliance/wasmtime/blob/main/cranelift/interpreter/src/step.rs)
  — a step-oriented shell with explicit control-flow results keeps interpreter state
  transitions centralized.

## Goals

1. Shrink the dialect-facing trait surface to the minimum needed for semantics.
2. Make all semantically visible state transitions flow through one effect algebra.
3. Use the same Lift algebra for both effect composition and error composition.
4. Keep frames, cursors, and stepping mechanics internal to the interpreter shell.
5. Start with single-stage concrete interpretation and leave room for abstract and
   multi-stage interpreters later.

## Non-Goals

- Multi-stage interpretation
- Abstract interpretation and fixpoint scheduling
- Debugging features such as fuel, breakpoints, and stepping hooks
- Derive macro design

## Core Contract

The design is organized around four roles:

| Role | Owns | Must not own |
| --- | --- | --- |
| `Interpretable<I>` | Per-statement semantics | Cursor movement, frame management, direct machine mutation |
| `Execute<I>` seed | Reusable sub-execution entrypoints such as block/region/function execution | A second effect algebra, direct machine mutation |
| `Interpreter` | Cursor, frames, SSA store, execution loop | Dialect-specific semantic decisions |
| Dialect `Machine` | Dialect-local state transitions for `Machine(DE)` | Interpreter control flow or frame state |

This yields four normative rules:

1. **Dialect semantics are effect-first.** If a state transition is semantically visible,
   ordered relative to other effects, replayable, or interpreter-dependent, it must be
   represented in `Effect<V, DE>`.
2. **Dialects do not mutate machine state directly.** The public dialect-facing API does
   not include `ProjectMut`. Stateful dialects emit `Effect::Machine(de)` and let the
   interpreter route that effect to the dialect machine.
3. **Seeds are reusable sub-execution executors, not effect variants.** A seed represents a
   stable interpreter-side execution boundary such as block, region, function, or a dialect-defined
   executable body kind. Per-operation orchestration should usually stay in `interpret()` and
   call shared seeds as needed.
4. **The interpreter shell owns execution mechanics.** Cursor advancement, block jumps, frame
   push/pop, and completion are interpreter responsibilities, even when a seed triggers them.

## Public Trait Surface

Dialect authors should only need to understand:

- [traits/machine.md](traits/machine.md) — stateful effect consumers
- [traits/interpretable.md](traits/interpretable.md) — per-statement semantics and the
  interpreter supertrait
- [traits/seed.md](traits/seed.md) — seeds for multi-step execution
- [traits/effects.md](traits/effects.md) — the effect algebra
- [traits/errors.md](traits/errors.md) — the error algebra
- [traits/lift_and_project.md](traits/lift_and_project.md) — sum/product composition

## Effect Algebra

The effect algebra is:

```rust
enum Effect<V, DE> {
    Advance,
    Stay,
    Jump(Block, SmallVec<[V; 2]>),
    BindValue(SSAValue, V),
    BindProduct(Product<ResultValue>, V),
    Return(V),
    Yield(V),
    Stop(V),
    Seq(SmallVec<[Self; 2]>),
    Machine(DE),
}
```

- `V` is the runtime value type.
- `DE` is the dialect-machine effect type.
- Dialects with no machine effects use `Infallible`, not `()`.

Two important consequences:

1. `Effect` describes observable interpreter state transitions only.
2. There is no `Execute(Seed)` variant. Seeds are invoked directly and return effects.

## Error Algebra

Errors mirror the effect composition story:

```rust
enum InterpError<DE> {
    Interpreter(InterpreterError),
    Dialect(DE),
}
```

- `InterpreterError` covers shell failures such as unbound SSA values, arity mismatches, or
  invalid stage resolution.
- `DE` covers dialect-level failures, including machine-consumption failures and
  dialect-specific semantic errors.
- Dialects with no custom errors use `Infallible`.

## Composition Model

Composition follows one rule consistently:

- Machines compose by **product**.
- Effects compose by **sum**.
- Errors compose by **sum**.
- Dialects compose by **sum**.

`Lift` is defined on the composite type. For effects and errors, only the dialect-specific
payload is transformed:

- `Lift<Effect<V, DEA>> for Effect<V, DEC>` rewrites only `Machine(de)`.
- `Lift<InterpError<EA>> for InterpError<EC>` rewrites only `Dialect(err)`.

This keeps composition mechanical and derivable for `#[wraps]` enums.

## Seeds

Seeds exist to package reusable interpreter-owned sub-execution:

- enter a block with concrete block arguments
- enter a region and follow control-flow edges
- push a frame and run a callee body
- resolve a staged call and delegate to function execution
- execute a dialect-defined body kind such as a graph body

The key boundary is:

- `Interpretable<I>` decides whether to invoke shared sub-executors and how to interpret
  their results for the current operation.
- `Execute<I>` implements reusable mechanics for a stable sub-execution entrypoint.
- Operation-specific control logic that is not reused should remain inline in `interpret()`.

Seeds are therefore a reusable control abstraction, not an extension point in the effect algebra.

## Initial Implementation Target

[single_stage.md](single_stage.md) is the first concrete interpreter:

- one language
- one stage
- concrete execution
- no fixpoint loop

It is the proving ground for the trait surface and the effect/error composition rules.

## Comparison with interpreter-2

| Concept | interpreter-2 | interpreter-3 |
| --- | --- | --- |
| Dialect-facing surface | Many traits (`Driver`, `Exec`, `Invoke`, `ResolveCallee`, `Fuel`, `Breakpoints`, ...) | `Interpretable`, `Interpreter`, `Execute`, `Machine` |
| Observable state changes | Split across helpers and traits | Unified in `Effect<V, DE>` |
| Error composition | Multiple ad-hoc `From` paths | Unified in `InterpError<DE>` |
| Stateful dialect access | Direct mutation plus effect hooks | Effect-first via `Machine(DE)` |
| Seed integration | Entangled with machine traits | Reusable sub-execution executors |
| Frame/cursor model | Publicly exposed | Internal to interpreter |

## Deferred

- Abstract interpreter design
- Multi-stage interpreter design
- Derive support for `Lift` and `Interpretable`
- Debug hooks layered on top of the core shell
