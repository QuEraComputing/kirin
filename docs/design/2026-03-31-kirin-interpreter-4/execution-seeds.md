# Execution Seeds

## Motivation

In interpreter-3, all IR traversal was hardcoded into the shell's `inherit`
method. Dialect authors could not customize how they traverse statement bodies.

Interpreter-4 replaces hardcoded traversal with **user-definable cursor types**
that implement `Execute<I>`. Each cursor encapsulates a traversal strategy.
A single flat driver loop pops cursor entries and dispatches structural effects.

## Design: Cursor Types as Execution Seeds

There is no separate "seed" concept. The cursor's initial state IS the seed —
construction encodes the starting point (block, args, metadata), mutation tracks
traversal progress.

### Core traits

```rust
/// User-definable cursor type. Tracks traversal state, returns effects.
pub trait Execute<I: Interpreter> {
    fn execute(&mut self, interp: &mut I) -> Result<<I as Machine>::Effect, I::Error>;
}
```

`Execute` impls are for concrete interpreter types (e.g., `SingleStage<...>`),
not generic `I: Interpreter`. The trait genericity enables sum-enum dispatch
for composite cursor enums, not interpreter-polymorphic cursor reuse.

### Effect algebra

```rust
pub enum Action<V, R = (), C = ()> {
    Advance,                                          // local — handled inside execute
    Jump(Block, Vec<V>),                              // local — handled inside execute
    Return(V),                                        // structural — driver pops frame
    Yield(V),                                         // structural — driver sets pending yield
    Push(C),                                          // structural — driver stacks cursors
    Call(SpecializedFunction, Vec<V>, Vec<ResultValue>), // structural — driver pushes frame
    Delegate(R),                                      // structural — driver delegates to machine
}
```

Local effects (Advance, Jump) are consumed inside `execute`'s inner loop.
Structural effects bubble to the driver.

### Global cursor stack

The interpreter has a single global `cursors: Vec<C>` separate from the frame
stack. The frame stack holds SSA value bindings. The cursor stack holds
traversal state. These are orthogonal.

**Why global?** Per-frame cursor stacks break when a cursor crosses frame
boundaries. With a global stack, cursor entries naturally mirror the nesting
structure. Return consumes the callee's entries; the parent's entries sit below.

**Why Call is a driver-handled effect?** The frame stack is interpreter-internal
state. Cursors return `Call` effects; the driver pushes/pops frames. This
eliminates `FunctionCursor` for standard calls.

### Driver constraint

The driver requires `C: Execute<Self> + Lift<BlockCursor<V>>`. The `Lift` bound
exists because the `Call` handler creates a `BlockCursor` for the callee's
entry block and lifts it into `C`. All cursor entry types must have a
`Block(BlockCursor<V>)` variant (or be `BlockCursor<V>` itself).

## Cursor Types

### `BlockCursor<V>`

Linear traversal through a single block. Handles Advance/Jump internally.
Returns structural effects (Return, Yield, Push, Call) to the driver.

```rust
pub struct BlockCursor<V> {
    block: Block,
    current: Option<Statement>,
    results: Vec<ResultValue>,
    args: Option<Vec<V>>,  // bound on first execute
}
```

### Composition

Cursor types compose as sum enums, following the same pattern as dialects and
effects:

```rust
enum ScfCursor<V> {
    Block(BlockCursor<V>),
    Region(RegionCursor<V>),  // deferred
    For(ForCursor<V>),        // deferred
}
```

`Lift` composes: `BlockCursor<V>` lifts into `ScfCursor::Block(...)`.

### Custom cursors

Domain-specific crates define their own cursor types:

```rust
impl Execute<SingleStage<...>> for TensorGraphCursor {
    fn execute(&mut self, interp: &mut SingleStage<...>) -> Result<Effect, Error> {
        // Custom graph traversal
    }
}
```

## Two Execution Paths

### Deferred (cursor stack — primary)

Dialect authors return effects that the driver handles after `interpret` returns.
The cursor stack manages all nesting with zero Rust recursion.

- `Action::Push(cursor)` — inline body execution (scf.if, scf.for)
- `Action::Call(callee, args, results)` — function invocation
- `Action::Return(v)` — function return
- `Action::Yield(v)` — inline body completion
- `Action::Jump(block, args)` — intra-block CFG traversal (handled by cursor)

### Synchronous (future)

Execution seed methods (`exec_block`, `exec_region`, `invoke`) on the
interpreter that dialect authors call during `interpret`. These are convenience
wrappers that internally use the cursor stack. Deferred to future work — the
deferred path handles all current use cases.

## Callee Resolution

Minimal for this iteration. The driver resolves callees via:

```rust
impl SingleStage<...> {
    fn push_call_frame(&mut self, callee: SpecializedFunction, ...) {
        // callee → SpecializedFunctionInfo → body Statement → regions → entry block
    }
}
```

Callee query builder (`callee::Query` from interpreter-2) is deferred.

## Interaction with Abstract Interpretation

Cursor types are the key abstraction for making traversal portable:

- Concrete `BlockCursor`: linear traversal, follow jumps, return on structural effect
- Abstract `BlockCursor` (future): run to fixpoint, return joined abstract value
- Concrete `ForCursor` (future): iterate loop body via Push/Yield cycle
- Abstract `ForCursor` (future): widening/narrowing until fixpoint

The dialect author writes ONE `Interpretable` impl. The cursor type determines
the traversal strategy.

## Backlog

- `RegionCursor<V>` — CFG traversal with proper region semantics
- `ForCursor<V>` — loop iteration via Push/Yield cycle
- Synchronous execution seed methods (`exec_block`, `exec_region`, `invoke`)
- Callee query builder
- `#[derive(Execute)]` for composite cursor enums
- Graph cursors: `DiGraphCursor`, `UnGraphCursor`
- Abstract interpreter cursors: fixpoint block/region execution
- `IsPush` marker trait (defined, not yet used by driver)
- Generalize cursor type visibility via `type CursorEntry` on `Interpreter`
