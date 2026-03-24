# Runtime Model

## Public Trait Family

The new crate should expose focused traits instead of one block-centric
supertrait:

- `ValueStore`
- `StageAccess<'ir>`
- `ExecStatement<'ir>`
- `ExecBlock<'ir>`
- `ExecRegion<'ir>`
- `VisitDiGraph<'ir>`
- `VisitUnGraph<'ir>`
- `CallExecutor<'ir>`
- `CallableBody<'ir, I>`
- `DebugDriver<'ir>`
- `ConsumeResult<'ir, I>`

An internal umbrella trait may compose the concrete runtime, but dialect
authors should be able to depend on the narrowest capability they actually
need.

## Stage Dispatch And Typed-Stage API

The current concrete interpreter depends on stage-dynamic dispatch for
stepping, advancing, and cross-stage call entry. `kirin-interpreter-2` should
preserve that capability explicitly.

The concrete runtime should:

- precompute a dispatch cache keyed by `CompileStage`
- attach the resolved dispatch entry to each active frame
- use the frame-cached entry during stepping and call entry

`StageAccess<'ir>` should also keep the typed-stage helpers analogous to:

- `in_stage::<L>()`
- `try_in_stage::<L>()`
- `with_stage(stage)`

The resulting `Staged<'a, 'ir, I, L>` facade should remain the ergonomic host
API for typed execution and manual control. In the concrete runtime it should
continue to expose operations analogous to:

- `step`
- `advance`
- `run`
- `run_until_break`
- `call`

## Callable Bodies

The new runtime needs an explicit abstraction for the callee body:

```rust
CallableBody<'ir, I>
```

This is the runtime-facing trait that decides how a callable body enters
execution or otherwise evaluates a call boundary.

The separation matters because:

- callsites own outward result conventions through `ConsumeResult`
- callable bodies own how nested execution begins
- the framework still needs a uniform place for standard body behavior such as
  SSA CFG regions

A blanket CFG callable-body path should remain part of the design.

## Interpreter-Global State

The concrete interpreter should remain parameterized by interpreter-global state
`G`, with accessors equivalent to `global()` and `global_mut()`.

This state is distinct from SSA value storage and should stay available for:

- embedding environments
- external runtime resources
- debugger/session state
- dialect-specific shared runtime state

## Effects And Stop Reasons

Semantic execution effects stay separate from debugger/runtime stop reasons.

The shared semantic effect algebra for v1 is:

```rust
enum ExecEffect<V> {
    Continue,
    Jump {
        block: Block,
        args: Product<V>,
    },
    Call {
        callee: SpecializedFunction,
        stage: CompileStage,
        args: Product<V>,
    },
    Return(V),
    Yield(V),
    Fork(SmallVec<[(Block, Product<V>); 2]>),
}
```

Debugger and driver status should use a separate channel such as `RunStatus`
plus `StopReason`.

## Internal Cursor Model

`ExecutionCursor` is internal in v1. It is the resumable machine state used by
the runtime loop. `ExecutionLocation` is the public/debug projection of that
state.

The cursor should be a closed enum by execution shape:

```rust
enum ExecutionCursor {
    Block(BlockCursor),
    Region(RegionCursor),
    DiGraph(DiGraphCursor),
    UnGraph(UnGraphCursor),
}
```

`ExecutionLocation` remains statement-based in v1:

```rust
enum ExecutionLocation {
    BeforeStatement(Statement),
    AfterStatement(Statement),
}
```

This keeps breakpoints uniform across blocks and graphs while preserving strong
cursor invariants.

## Region And Graph Execution

`ExecRegion` is a first-class execution trait, not merely an entry-block lookup
helper. It owns region-level stepping and scheduling. `ExecBlock` stays focused
on linear within-block execution.

Graph support should start at visitation, not at one universal graph execution
policy:

- `VisitDiGraph`
- `VisitUnGraph`

These traits provide framework-level visitation and state hooks without forcing
circuits, dataflow graphs, or future graph-like dialects through one baked-in
scheduler.

## Runtime Control Surfaces

The concrete runtime should preserve the control surfaces the old concrete
interpreter already provides:

- instruction/fuel budgeting
- maximum call-frame depth
- explicit breakpoint configuration
- runtime stop reasons beyond breakpoints, including halt-like stops

These belong to the driver/runtime layer rather than to `ExecEffect<V>`, but
they are still part of the concrete interpreter abstraction.

## Shared Frame Storage

Reusable frame infrastructure is worth preserving:

- `Frame<V, X>`
- `FrameStack<V, X>`

`kirin-interpreter-2` should continue to model frame storage as shared
infrastructure, with the concrete interpreter supplying per-frame extra state
such as:

- current execution cursor
- stage-dynamic dispatch entry
- pending nested-execution consumer metadata

This keeps the runtime structure reusable for a future abstract interpreter.

## Runtime Loop

The runtime loop is a small-step engine over `ExecutionCursor` and the active
frame's cached dispatch entry:

1. project the current cursor to `ExecutionLocation`
2. ask the debug driver whether execution should stop
3. step according to the current cursor shape
4. obtain an `ExecEffect<V>`
5. apply the effect by updating cursors and frames
6. repeat until stop or completion

Conceptually:

```rust
loop {
    let location = cursor.location();

    if debug.should_stop(&location) {
        return Stopped(Breakpoint(location));
    }

    let effect = match cursor {
        ExecutionCursor::Block(..) => exec_block.step(...)?,
        ExecutionCursor::Region(..) => exec_region.step(...)?,
        ExecutionCursor::DiGraph(..) => visit_digraph.step(...)?,
        ExecutionCursor::UnGraph(..) => visit_ungraph.step(...)?,
    };

    apply_effect(effect)?;
}
```

For stage-polymorphic execution, the concrete runtime should resolve the typed
implementation from the frame's cached dispatch entry rather than rediscovering
it on every step.
