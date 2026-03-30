# Seed & Execute

Seeds are reusable interpreter-owned executors for stable sub-execution entrypoints.
They are justified when multiple operations, or the interpreter shell itself, need the same
execution kernel.

This is the symmetry point for executable body kinds: if a dialect introduces a new reusable
body or entrypoint abstraction, it may introduce a corresponding seed and summary/output type.

## Execute Trait

```rust
trait Execute<I: Interpreter> {
    type Output;

    fn execute(self, interp: &mut I) -> Result<Self::Output, I::Error>;
}
```

- `Self` is the seed type.
- `I` is the interpreter shell being driven.
- `Output` depends on the reusable executor boundary:
  - low-level seeds often return terminal interpreter effects
  - some seeds translate those terminals into ordinary effects for their callers

## What Seeds Are For

Seeds are the right abstraction when they define reusable sub-execution:

- stepping through a block until its terminator
- dispatching into a region and following block-to-block control flow
- pushing a frame, running a callee body, then handling its `Return`
- resolving a staged call before entering the callee
- executing a dialect-defined graph or other executable body kind

## What Seeds Are Not

Seeds are not:

- effect variants
- a separate composition algebra
- a direct machine-mutation escape hatch
- a dumping ground for imperative code that only one operation needs once

If a seed needs to cause a state transition, it does so by returning or consuming effects
through the interpreter.

## Built-in Seed Types

- `BlockSeed<V>` — Block + block args. Binds args, steps statements, returns the terminator.
- `RegionSeed<V>` — Region + region args. Enters the region and follows block control flow.
- `FunctionSeed<V>` — callee + args + result slots. Pushes a frame, runs the function, then
  translates `Return` into a regular effect for the caller.
- `StagedFunctionSeed<V>` — stage-aware function execution that delegates to `FunctionSeed`.

## Dialect-Defined Execution Boundaries

Dialect authors may define new seeds when they are introducing a reusable executable body kind,
not merely an operation with some imperative control logic.

Examples that may justify a seed:

- `GraphSeed<V>` for a graph body representation shared by multiple graph operations
- `SchedulerSeed<V>` for a reusable dialect-level task graph entrypoint

Examples that usually do not justify a seed:

- `IfSeed` for one `scf.if` operation
- `ForLoopSeed` for one `scf.for` operation

## When Not to Introduce a New Seed

Do not introduce a new seed merely because `interpret()` contains a loop or a match.

Prefer inline orchestration inside `interpret()` when:

- the logic belongs to one operation only
- the logic starts from the current operation and nowhere else
- the code can be expressed by invoking existing seeds such as `BlockSeed` or `FunctionSeed`

For example, `scf.if` should usually select a block, run `BlockSeed`, and interpret the returned
terminal inline. That does not justify an `IfSeed`.

## Terminal Effects

Low-level seeds usually return terminal effects:

- `Effect::Yield(v)` for structured control-flow bodies
- `Effect::Jump(block, args)` for CFG execution
- `Effect::Return(v)` for function bodies

Higher-level reusable seeds may inspect those terminals and translate them into ordinary effects.
`FunctionSeed` is the main example: it turns callee execution into the regular effect sequence
expected by a call operation.

```rust
impl<I: Interpreter> Execute<I> for FunctionSeed<I::Value>
where
    RegionSeed<I::Value>: Execute<I, Output = I::Effect>,
{
    type Output = Effect<I::Value, Infallible>;

    fn execute(self, interp: &mut I) -> Result<Self::Output, I::Error> {
        let terminal = RegionSeed::new(self.callee.body(), self.args).execute(interp)?;

        match terminal {
            Effect::Return(value) => Ok(
                Effect::BindProduct(self.results, value).then(Effect::Advance)
            ),
            _ => Err(InterpreterError::unsupported("expected return from callee").into()),
        }
    }
}
```

## Why Seeds Are Outside the Effect Algebra

Earlier drafts encoded seed execution inside `Effect` itself. This design rejects that.

Reasons:

1. Seeds are control programs, not observable state transitions.
2. Keeping them outside `Effect` avoids making the algebra interpreter-specific.
3. Only reusable sub-execution should become a seed; operation-specific logic stays in `interpret()`.
4. `Lift` stays focused on dialect payload composition instead of control orchestration.

## Invariants

1. Seeds may drive the interpreter, but they do not own interpreter state.
2. Seeds may not mutate dialect machine state except through consumed or returned effects.
3. A seed should return the smallest useful output to its caller.
4. If a seed is only used by one operation and does not define a reusable entrypoint, it
   probably should not exist.
