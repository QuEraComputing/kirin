# Execution Seeds

## Motivation

In interpreter-3, all IR traversal was hardcoded into the shell's `inherit`
method. Dialect authors could not customize how they traverse statement bodies.
Execution seeds fix this by making common traversal patterns available as
**interpreter methods** that dialect authors call during `interpret`.

## Two Execution Paths

### Synchronous (inline execution seeds)

Dialect authors call execution methods on the interpreter during `interpret`.
The interpreter pushes a cursor, runs the body, collects the result, pops the
cursor, and returns to the dialect author.

```rust
let result = interp.exec_block(self.body_block, &args)?;
```

Use for:
- structured control flow bodies (`scf.if`, `scf.for`)
- inline function invocation when the dialect wants the result immediately
- any body where the dialect author controls the iteration

### Deferred (returned effects)

Dialect authors return an effect that tells the shell to handle cursor/frame
changes after `interpret` returns.

```rust
Ok(CfEffect::Jump(self.target, args))
```

Use for:
- intra-region control flow jumps (branch, cond_branch)
- function calls that should be handled by the shell's call stack
- return/yield from the current function
- stop signals

## Execution Seed Methods

The interpreter should provide these methods. Concrete and abstract interpreters
give different implementations.

### Block execution

```rust
fn exec_block(
    &mut self,
    block: Block,
    args: &[Self::Value],
) -> Result<Self::Value, Self::Error>;
```

Concrete: push block cursor, run non-terminator statements linearly, read
terminator yield value, pop cursor, return the value.

Abstract: run block to fixpoint, return the joined abstract value.

### Region execution (CFG)

```rust
fn exec_region(
    &mut self,
    region: Region,
    args: &[Self::Value],
) -> Result<Self::Value, Self::Error>;
```

Concrete: push region cursor starting at entry block, follow Jump effects to
successor blocks, return when a Return/Yield effect is encountered.

Abstract: worklist-driven fixpoint over all reachable blocks in the region.

### Function invocation

```rust
fn invoke(
    &mut self,
    callee: SpecializedFunction,
    args: &[Self::Value],
) -> Result<Self::Value, Self::Error>;
```

Concrete: push a new frame for the callee, execute the entry region, pop the
frame, return the result.

Abstract: check summary cache, compute summary if missing, return the summary
result.

### Graph execution (future)

```rust
fn exec_digraph(
    &mut self,
    graph: DiGraph,
    args: &[Self::Value],
) -> Result<Self::Value, Self::Error>;

fn exec_ungraph(
    &mut self,
    graph: UnGraph,
    args: &[Self::Value],
) -> Result<Self::Value, Self::Error>;
```

These are deferred but the trait surface reserves room for them.

## Trait Surface

Execution seeds should be methods on a trait that the interpreter implements:

```rust
trait ExecSeed {
    type Value: Clone;
    type Error;

    fn exec_block(
        &mut self,
        block: Block,
        args: &[Self::Value],
    ) -> Result<Self::Value, Self::Error>;

    fn exec_region(
        &mut self,
        region: Region,
        args: &[Self::Value],
    ) -> Result<Self::Value, Self::Error>;

    fn invoke(
        &mut self,
        callee: SpecializedFunction,
        args: &[Self::Value],
    ) -> Result<Self::Value, Self::Error>;
}
```

Or these could be methods directly on `Interpreter` as provided methods.
The exact factoring is an implementation decision — the key constraint is that
dialect authors can call them from within `interpret` via `&mut I`.

Whether `ExecSeed` is a separate trait or part of `Interpreter` depends on
whether we want dialect authors to opt into execution seed requirements:

```rust
// Option A: separate trait, opt-in
impl<I: Interpreter + ExecSeed> Interpretable<I> for ScfIf { ... }

// Option B: part of Interpreter, always available
impl<I: Interpreter> Interpretable<I> for ScfIf { ... }
```

Option B is simpler for dialect authors. Option A is more modular. For the MVP,
Option B is preferred — `Interpreter` includes execution seed methods.

## Callee Resolution

Function resolution belongs to the dialect author but the framework provides
query builder tooling. This carries forward from both kirin-interpreter (v1) and
interpreter-3 design.

The query builder is a method on the interpreter:

```rust
fn resolve_callee(
    &self,
    target: Symbol,
    args: &[Self::Value],
) -> Result<SpecializedFunction, Self::Error>;
```

Or as a builder for more complex resolution:

```rust
let callee = interp.callee()
    .symbol(self.target())
    .stage(stage_id)                     // optional, defaults to current
    .specialization(callee::UniqueLive)  // optional, defaults to UniqueLive
    .resolve()?;
```

For the MVP, a single `resolve_callee` method is sufficient. The builder can
be added when more resolution modes are needed.

## Custom Traversal

Execution seeds are helpers, not the only option. A dialect author can always
implement custom body traversal by directly working with the IR and the
interpreter:

```rust
impl<I: Interpreter> Interpretable<I> for CustomLoop {
    type Effect = ();
    type Error = I::Error;

    fn interpret(&self, interp: &mut I) -> Result<(), I::Error> {
        loop {
            let result = interp.exec_block(self.body, &[])?;
            let cond = interp.read(self.condition)?;
            if !cond.is_truthy() {
                interp.write(self.result, result)?;
                break;
            }
        }
        Ok(())
    }
}
```

This is the modularization that interpreter-3 lacked: the dialect author
controls the iteration pattern while the framework provides the block execution
primitive.

## Execution Seeds vs Shell Control

The split between execution seeds and shell control (effects):

| Mechanism | Owned by | Timing | Example |
|-----------|----------|--------|---------|
| `exec_block` | dialect author | synchronous | `scf.if`, `scf.for` body execution |
| `exec_region` | dialect author | synchronous | inline region execution |
| `invoke` | dialect author | synchronous | inline function call |
| `Effect::Jump` | shell | deferred | `cf.branch`, `cf.cond_branch` |
| `Effect::Call` | shell | deferred | function call with shell-managed frames |
| `Effect::Return` | shell | deferred | function return |
| `Effect::Stop` | shell | deferred | halt execution |

The synchronous path uses the interpreter's cursor/frame stack internally but
the dialect author sees it as a simple call-and-return. The deferred path
returns an effect that the shell processes after `interpret` completes.

## Interaction with Abstract Interpretation

Execution seeds are the key abstraction that makes `Interpretable` portable
across concrete and abstract interpreters:

- Concrete `exec_block`: push cursor, run linearly, return value
- Abstract `exec_block`: run to fixpoint, return joined abstract value
- Concrete `invoke`: push frame, execute, pop frame, return value
- Abstract `invoke`: check/compute summary, return abstract result

The dialect author writes ONE `Interpretable` impl. The interpreter type
determines the execution strategy.

For deferred effects, the abstract interpreter handles them differently:
- `CfEffect::Jump(block, args)` → enqueue block in worklist
- `CfEffect::Fork(targets)` → enqueue all targets (undecidable branch)
- `FuncEffect::Call(...)` → compute/lookup summary
- `FuncEffect::Return(v)` → join into function summary

This is handled in the interpreter's `consume_effect`, not by the dialect
author.
