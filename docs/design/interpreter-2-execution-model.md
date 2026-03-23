# Kirin Interpreter 2 Execution Model Design

## Summary

This design defines the core execution model for a new crate, `kirin-interpreter-2`.
The goal is to replace the current block-centric interpreter framework with a
shape-aware runtime that treats `Block`, `Region`, `DiGraph`, and `UnGraph` as
first-class execution shapes while keeping result-convention policy in dialect
implementations rather than in the framework.

The concrete stack interpreter is the first target. The trait and data-model
boundaries should remain suitable for a future abstract interpreter, but no
abstract interpreter implementation is part of this design.

## Goals

- Make execution shape-aware instead of centering the entire framework on block
  execution.
- Keep recursion and nested execution on an explicit interpreter-managed frame
  stack rather than Rust call frames.
- Separate semantic execution effects from debugger/runtime stop reasons.
- Keep `ExecutionLocation` statement-based for v1 so breakpoints work uniformly
  across blocks and graphs.
- Make multi-result conventions dialect-owned rather than enforced as a global
  runtime-value requirement.
- Reuse raw `kirin_ir::Product<V>` in public interpreter APIs where a structural
  list of values is needed.

## Non-Goals

- Porting behavior from `kirin-interpreter` incrementally.
- Defining a public fully generic machine abstraction.
- Defining a core framework trait for implicit tuple/product packing or
  unpacking.
- Implementing abstract interpretation in this first crate.

## Key Decisions

### 1. Public trait family, internal umbrella

The new crate should expose a small family of focused traits instead of one
block-centric supertrait:

- `ValueStore`
- `StageAccess<'ir>`
- `ExecStatement<'ir>`
- `ExecBlock<'ir>`
- `ExecRegion<'ir>`
- `VisitDiGraph<'ir>`
- `VisitUnGraph<'ir>`
- `CallExecutor<'ir>`
- `DebugDriver<'ir>`
- `ConsumeResult<'ir, I>`

An internal umbrella trait may compose the concrete runtime, but dialect authors
should be able to depend on the narrowest capability they actually need.

### 2. Effects and runtime events are separate

Semantic execution effects are distinct from debugger/runtime stop reasons.

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

Debugger and driver status should use a separate channel, for example through a
runtime `RunStatus` and `StopReason`.

### 3. Execution state is an internal closed cursor enum

`ExecutionCursor` is internal in v1. It is the resumable machine state used by
the runtime loop. `ExecutionLocation` is the public/debug projection of that
state.

`ExecutionCursor` should be a closed enum by execution shape:

```rust
enum ExecutionCursor {
    Block(BlockCursor),
    Region(RegionCursor),
    DiGraph(DiGraphCursor),
    UnGraph(UnGraphCursor),
}
```

This gives strong invariants and keeps the cursor aligned with Kirin IR body
shapes. `RegionCursor` is included from day one because new region kinds are
expected soon.

`ExecutionLocation` remains statement-based in v1:

```rust
enum ExecutionLocation {
    BeforeStatement(Statement),
    AfterStatement(Statement),
}
```

### 4. Region is a first-class execution shape

`ExecRegion` is not just an entry-block lookup helper. It owns region-level
stepping and scheduling. `ExecBlock` stays focused on linear within-block
execution.

This allows current CFG-like regions to work naturally while leaving a home for
future region kinds that are not just "find entry block and run blocks linearly".

### 5. Graph support starts at visitation, not generic execution semantics

`VisitDiGraph` and `VisitUnGraph` provide framework-level graph visitation and
state hooks. They should not impose one universal graph scheduler or execution
semantics.

This keeps graph bodies first-class without forcing circuits, dataflow graphs,
and future graph-like dialects through one baked-in traversal rule.

### 6. Raw `Product<V>` is used directly for structural value lists

The public interpreter protocol should use raw `kirin_ir::Product<V>` directly
for structural lists of runtime values such as:

- block arguments
- call arguments
- fork target arguments

No extra wrapper type is introduced for these lists.

### 7. `Return` and `Yield` stay single-valued

The semantic execution protocol uses `Return(V)` and `Yield(V)`, not
`Return(Product<V>)` or `Yield(Product<V>)`.

This preserves the intended design that multiple outward results are a dialect
convention, often expressible as sugar over one product-valued semantic result,
rather than a second framework-level result transport mechanism.

### 8. No global `ProductValue`-style requirement in the core

The new core interpreter must not impose a global runtime-value trait for
packing or unpacking product values.

If a dialect wants an implicit multi-result convention, such as using a value
enum variant like `Tuple(Product<Self>)`, the dialect author handles that logic
in the relevant `Interpretable` and `ConsumeResult` implementations.

The framework owns execution mechanics, not value-convention policy.

### 9. Result consumption is generic and dialect-owned

Nested execution boundaries are handled through a generic consumer trait:

```rust
trait ConsumeResult<'ir, I: Interpreter<'ir>> {
    fn consume_result<L>(
        &self,
        interp: &mut I,
        value: I::Value,
    ) -> Result<(), I::Error>
    where
        I::StageInfo: HasStageInfo<L>,
        I::Error: From<InterpreterError>,
        L: Interpretable<'ir, I> + 'ir;
}
```

This trait is intentionally generic rather than call-specific or yield-specific.
It should work for any statement that starts nested execution and later needs to
map one semantic result value back into outward-facing results.

Examples include:

- function call statements
- `scf.if`
- `scf.for`
- future compound graph-node operations

In v1, `ConsumeResult` should be implemented only by statement definitions.

### 10. `Call` remains an effect so recursion uses the interpreter frame stack

`Call` must remain in `ExecEffect`. Making call execution a synchronous Rust
function returning `V` would move recursion to Rust call frames and would weaken
stepping, debugging, and explicit control over nested execution.

Instead, the runtime handles `ExecEffect::Call` by:

1. storing the pending consumer statement and resume cursor
2. pushing a new callee frame
3. running the callee on the interpreter-managed frame stack
4. receiving `Return(V)`
5. popping the callee frame
6. restoring the caller
7. invoking `ConsumeResult` on the pending consumer statement

This keeps recursion and nested execution explicit and debugger-friendly.

## Runtime Loop

The runtime loop is a small-step engine over `ExecutionCursor`:

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

## Dialect Examples

### Single-result call convention

A dialect can define a call op that expects exactly one outward result and write
the returned value directly in `ConsumeResult`.

No packing or unpacking support is needed.

### Multi-result call convention

A dialect can define a call op with multiple outward result slots and implement
its own unpacking logic in `ConsumeResult`.

For example, a dialect-specific value enum may contain a
`Tuple(Product<Self>)` variant. The dialect then unpacks that variant in
`ConsumeResult` and writes each outward result slot itself.

If the returned value does not match the dialect's convention, the dialect
raises an error there.

### `Return(SSAValue)` or `Yield(SSAValue)`

If a dialect chooses a single SSA operand form, its `Interpretable`
implementation simply reads that value and emits `ExecEffect::Return(v)` or
`ExecEffect::Yield(v)`.

Any outward arity adaptation is handled by the consuming statement, not by the
framework.

### `Return(Vec<SSAValue>)` or `Yield(Vec<SSAValue>)`

If a dialect chooses an explicit multi-operand form, its `Interpretable`
implementation is responsible for packing those values into one semantic `V`
before emitting `Return(v)` or `Yield(v)`.

Again, that packing policy is dialect-owned.

## Error Ownership

Framework errors cover execution-mechanics failures, for example:

- invalid cursor transitions
- missing stage or frame
- invalid block-argument arity at a control transfer boundary
- unexpected `Return` or `Yield` at the wrong runtime boundary

Dialect errors cover result-convention failures, for example:

- a callsite cannot unpack a returned value into its outward result slots
- a return/yield op cannot pack multiple SSA operands into one semantic value
- a graph boundary consumer rejects the nested result value shape

This keeps ownership coherent: the framework handles control mechanics; the
dialect handles value meaning.

## Testing Strategy

### 1. Core runtime tests in `kirin-interpreter-2`

- recursive calls use the explicit frame stack
- pending consumer resumes the correct statement after `Return`
- yielded values resume the correct parent boundary
- breakpoint stop/resume works through `ExecutionLocation`
- `BlockCursor`, `RegionCursor`, `DiGraphCursor`, and `UnGraphCursor` follow
  correct transitions

### 2. Dialect-facing integration tests

Use small test dialects to prove that the same runtime supports multiple result
conventions:

- strict single-result convention
- implicit tuple/product convention using a dialect-owned `Tuple(Product<Self>)`
  value variant
- `scf.if` and `scf.for`-style nested execution using `ConsumeResult`

### 3. Graph-boundary tests

Once graph visitation lands:

- a compound graph node consumes nested execution results through
  `ConsumeResult`
- breakpoint locations remain statement-based inside graph execution

## Recommended Initial Scope

The first implementation of `kirin-interpreter-2` should include:

- the core trait family
- internal cursor model
- the concrete stack-based runtime
- block and region execution
- explicit call-stack handling through `ExecEffect::Call`
- statement-owned `ConsumeResult`

Graph visitation traits should be designed in this crate from the start, but
their first concrete execution behavior can land after block/region execution is
stable.

## Follow-Up Planning

Implementation planning should focus on:

1. core runtime data structures and traits
2. explicit frame and pending-consumer state
3. block and region stepping
4. concrete call/return handling
5. adapting one or two representative dialects
6. adding graph visitation surfaces without overcommitting to graph scheduling
