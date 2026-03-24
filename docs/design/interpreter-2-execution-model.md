# Kirin Interpreter 2 Execution Model Design

## Summary

This design defines the core execution model for a new crate,
`kirin-interpreter-2`, plus the derive-package surface needed to target that
runtime ergonomically from dialect crates. The goal is to replace the current
block-centric interpreter framework with a shape-aware runtime that treats
`Block`, `Region`, `DiGraph`, and `UnGraph` as first-class execution shapes
while keeping result-convention policy in dialect implementations rather than in
the framework.

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
- Retrofitting `kirin-derive-interpreter` in place to target the new runtime.
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
- `CallableBody<'ir, I>`
- `DebugDriver<'ir>`
- `ConsumeResult<'ir, I>`

An internal umbrella trait may compose the concrete runtime, but dialect authors
should be able to depend on the narrowest capability they actually need.

The public API should also retain a typed-stage facade through `Staged<'a, ...>`
for ergonomic host entrypoints and typed-stage execution.

### 2. A separate derive package targets the new runtime

The new runtime should have its own derive package,
`kirin-derive-interpreter-2`, rather than extending
`kirin-derive-interpreter` in place.

The macro surface should be:

- `#[derive(Interpretable)]`
- `#[derive(ConsumeResult)]`
- `#[derive(CallableBody)]`
- `#[derive(SSACFGCallableBody)]`

These names intentionally preserve the generic semantic names where they still
fit, but they do not carry forward old runtime-specific names such as
`CallSemantics` or `SSACFGRegion`.

The derive-specific attribute model should be:

- reuse `#[wraps]`
- reuse `#[callable]`
- reuse `#[interpret(...)]` for derive-local options, including interpreter
  crate path override
- add `#[body]` for concrete body-specific derives

Forwarding rules should remain strict and predictable:

- `Interpretable` derive is wrapper-only and forwards all `#[wraps]`
- `ConsumeResult` derive is wrapper-only and forwards all `#[wraps]`
- `CallableBody` derive is wrapper-only and forwards only `#[wraps]` selected
  by `#[callable]`
- `SSACFGCallableBody` uses the same `#[callable]` selection rule for wrapper
  forwarding, while concrete structs require exactly one `#[body]` field of type
  `Region`

This keeps `#[wraps]` as the marker of semantic equivalence, keeps `#[callable]`
as the marker of callable-body forwarding, and avoids introducing redundant
selector attributes.

### 3. Stage-dynamic dispatch is explicit and cached

The current concrete interpreter relies on stage-dynamic dispatch for stepping,
advancing, and cross-stage call entry. `kirin-interpreter-2` needs an explicit
equivalent.

The concrete runtime should precompute a dispatch cache keyed by
`CompileStage`, then attach the resolved dispatch entry to each active frame.
This keeps runtime lookup O(1) while still allowing:

- cross-stage calls
- cross-stage recursion
- host-driven `call(spec, stage, args)` entrypoints
- stage-polymorphic stepping without repeated trait dispatch

This is an internal runtime abstraction, but it is required for the concrete
stack interpreter to preserve current functionality.

### 4. The typed-stage facade stays public

`StageAccess<'ir>` should continue to provide typed-stage helpers analogous to:

- `in_stage::<L>()`
- `try_in_stage::<L>()`
- `with_stage(stage)`

The resulting `Staged<'a, 'ir, I, L>` handle should remain the ergonomic typed
API for host-side entrypoints and manual interpreter control. In the concrete
runtime, it should continue to expose operations analogous to:

- `step`
- `advance`
- `run`
- `run_until_break`
- `call`

This keeps host code concise and preserves the current typed-stage execution
style.

### 5. Callable bodies have their own abstraction

The new design needs an explicit abstraction for the *callee body*, not just the
callsite.

`CallableBody<'ir, I>` is the runtime-facing trait that decides how a callable
body enters execution or otherwise evaluates a call boundary. This is the v2
equivalent of the role currently played by `CallSemantics` and `SSACFGRegion`.

This separation is important because:

- callsites own outward result conventions via `ConsumeResult`
- callable bodies own how execution begins or how custom body kinds are entered
- the framework still needs a uniform place to attach blanket behavior for
  standard callable body shapes such as SSA CFG regions

A blanket implementation for CFG-style callable regions should remain part of
the design.

`SSACFGCallableBody` is the derive-time counterpart of that blanket path. It is
separate from `CallableBody` so CFG-specific behavior does not leak into the
generic callable-body abstraction.

### 6. Interpreter-global state remains first-class

The concrete interpreter should remain parameterized by interpreter-global state
`G`, with accessors equivalent to `global()` and `global_mut()`.

This state is separate from SSA value storage and should stay available for:

- embedding environments
- external runtime resources
- debugger/session state
- dialect-specific shared state that is not tied to one call frame

Whether this is exposed through a dedicated trait or inherent methods can be
settled during implementation planning, but the design should preserve the
capability.

### 7. Effects and runtime events are separate

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

### 8. Execution state is an internal closed cursor enum

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

### 9. Region is a first-class execution shape

`ExecRegion` is not just an entry-block lookup helper. It owns region-level
stepping and scheduling. `ExecBlock` stays focused on linear within-block
execution.

This allows current CFG-like regions to work naturally while leaving a home for
future region kinds that are not just "find entry block and run blocks linearly".

### 10. Graph support starts at visitation, not generic execution semantics

`VisitDiGraph` and `VisitUnGraph` provide framework-level graph visitation and
state hooks. They should not impose one universal graph scheduler or execution
semantics.

This keeps graph bodies first-class without forcing circuits, dataflow graphs,
and future graph-like dialects through one baked-in traversal rule.

### 11. Raw `Product<V>` is used directly for structural value lists

The public interpreter protocol should use raw `kirin_ir::Product<V>` directly
for structural lists of runtime values such as:

- block arguments
- call arguments
- fork target arguments

No extra wrapper type is introduced for these lists.

### 12. `Return` and `Yield` stay single-valued

The semantic execution protocol uses `Return(V)` and `Yield(V)`, not
`Return(Product<V>)` or `Yield(Product<V>)`.

This preserves the intended design that multiple outward results are a dialect
convention, often expressible as sugar over one product-valued semantic result,
rather than a second framework-level result transport mechanism.

### 13. No global `ProductValue`-style requirement in the core

The new core interpreter must not impose a global runtime-value trait for
packing or unpacking product values.

If a dialect wants an implicit multi-result convention, such as using a value
enum variant like `Tuple(Product<Self>)`, the dialect author handles that logic
in the relevant `Interpretable` and `ConsumeResult` implementations.

The framework owns execution mechanics, not value-convention policy.

### 14. Result consumption is generic and dialect-owned

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

### 15. `Call` remains an effect so recursion uses the interpreter frame stack

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

### 16. Runtime control surfaces stay explicit

The concrete interpreter should continue to expose the runtime-control features
that exist today. In particular, the design should preserve:

- instruction/fuel budgeting
- maximum call-frame depth
- explicit breakpoint configuration
- runtime stop reasons beyond breakpoints, including dialect-driven halt-like
  stops

These controls belong to the concrete runtime and driver layer rather than to
`ExecEffect<V>`, but they are part of the concrete interpreter abstraction and
should be called out explicitly.

### 17. Shared frame storage remains reusable

The existing split between a reusable `Frame<V, X>` / `FrameStack<V, X>` kernel
and interpreter-specific per-frame extra state is worth preserving.

`kirin-interpreter-2` should continue to model frame storage as reusable shared
infrastructure, with the concrete interpreter supplying extra per-frame state
such as:

- current execution cursor
- stage-dynamic dispatch entry
- pending nested-execution consumer metadata

This matters because the new crate is still intended to share runtime structure
with a future abstract interpreter even if only the concrete interpreter is
implemented initially.

## Runtime Loop

The runtime loop is a small-step engine over `ExecutionCursor` and the active
frame's already-resolved dispatch entry:

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

### 2. Derive-package tests

`kirin-derive-interpreter-2` should have its own focused test coverage:

- snapshot or token-based tests for generated code
- compile-pass tests for the happy paths
- compile-fail tests for invalid combinations such as:
  missing `#[wraps]` on forwarding derives,
  missing `#[callable]` on callable-body forwarding derives,
  invalid `#[body]` usage on `SSACFGCallableBody`

These tests should land before downstream dialect migration begins.

### 3. Dialect-facing integration tests

Use small test dialects to prove that the same runtime supports multiple result
conventions:

- strict single-result convention
- implicit tuple/product convention using a dialect-owned `Tuple(Product<Self>)`
  value variant
- `scf.if` and `scf.for`-style nested execution using `ConsumeResult`

### 4. Graph-boundary tests

Once graph visitation lands:

- a compound graph node consumes nested execution results through
  `ConsumeResult`
- a small toy language can define a computational-graph statement with a
  `DiGraph` body, and its outward result should match a reference execution of
  the equivalent computation through plain block execution
- this comparison is output-level only; it does not require implementing a
  second block-based version of the same language
- breakpoint locations remain statement-based inside graph execution

## Recommended Initial Scope

The first implementation of `kirin-interpreter-2` should include:

- the core trait family
- stage-dynamic dispatch cache and per-frame dispatch entries
- the typed `Staged<'a, ...>` facade
- internal cursor model
- reusable frame and frame-stack infrastructure
- the concrete stack-based runtime
- interpreter-global state support
- fuel, max-depth, breakpoint, and halt control surfaces
- block and region execution
- callable-body abstraction with a blanket CFG-region path
- explicit call-stack handling through `ExecEffect::Call`
- statement-owned `ConsumeResult`

Graph visitation traits should be designed in this crate from the start, but
their first concrete execution behavior can land after block/region execution is
stable.

Before downstream dialect migration begins, the workspace should also have a
separate `kirin-derive-interpreter-2` crate implementing the approved derive
surface for the new runtime.

## Follow-Up Planning

Implementation planning should focus on:

1. core runtime data structures and traits
2. explicit frame and pending-consumer state
3. block and region stepping
4. concrete call/return handling
5. graph visitation surfaces without overcommitting to graph scheduling
6. a separate `kirin-derive-interpreter-2` package
7. migrating one or two representative dialects only after the new derive
   package is finished
