# Interpreter Framework Simplification

## Goals

Simplify the kirin-interpreter crate's public API to:
- Minimize symbols downstream developers need to import or learn
- Ensure API names are self-explanatory
- Eliminate overlapping logic and duplicated method variants
- Reduce conceptual model complexity and learning curve
- Maintain full power for both dialect authors and interpreter builders

## Trait Hierarchy

### Before

Five traits, with `EvalBlock` as a separate trait that dialect authors must add to bounds:

```
Interpreter<'ir>           — 14 methods (core + stage + dispatch + builders)
EvalBlock<'ir, L>          — separate trait for block execution
Interpretable<'ir, I, L>   — dialect authors implement
EvalCall<'ir, I, L>        — function dialects implement
SSACFGRegion               — marker for blanket EvalCall impl
```

### After

Four traits. `EvalBlock` folded into `Interpreter`. `EvalCall` renamed to `CallSemantics`:

```
Interpreter<'ir>           — 7 required + 5 provided methods
Interpretable<'ir, I, L>   — dialect authors implement (unchanged)
CallSemantics<'ir, I, L>   — renamed from EvalCall, function dialects implement
SSACFGRegion               — unchanged, provides blanket CallSemantics impl
```

### Interpreter Trait (Revised)

```rust
pub trait Interpreter<'ir>: Sized + 'ir {
    type Value: Clone;
    type Error;
    type Ext: fmt::Debug;
    type StageInfo: StageMeta;

    // === Required (7) — interpreter builders implement these ===

    fn read(&self, value: SSAValue) -> Result<Self::Value, Self::Error>;
    fn write(&mut self, result: ResultValue, value: Self::Value) -> Result<(), Self::Error>;
    fn write_ssa(&mut self, ssa: SSAValue, value: Self::Value) -> Result<(), Self::Error>;
    fn pipeline(&self) -> &'ir Pipeline<Self::StageInfo>;
    fn active_stage(&self) -> CompileStage;

    fn bind_block_args<L: Dialect>(
        &mut self,
        stage: &'ir StageInfo<L>,
        block: Block,
        args: &[Self::Value],
    ) -> Result<(), Self::Error>;

    fn eval_block<L: Dialect>(
        &mut self,
        stage: &'ir StageInfo<L>,
        block: Block,
    ) -> Result<Continuation<Self::Value, Self::Ext>, Self::Error>;

    // === Provided (5) — come for free ===

    fn active_stage_info<L>(&self) -> &'ir StageInfo<L>
    where Self::StageInfo: HasStageInfo<L>, L: Dialect;

    fn resolve_stage_id<L: Dialect>(&self, stage: &StageInfo<L>) -> CompileStage;

    fn resolve_stage_info<L>(
        &self,
        stage_id: CompileStage,
    ) -> Result<&'ir StageInfo<L>, Self::Error>
    where Self::StageInfo: HasStageInfo<L>, L: Dialect, Self::Error: From<InterpreterError>;

    fn in_stage<L>(&mut self) -> Staged<'_, 'ir, Self, L>;
    fn with_stage<L: Dialect>(&mut self, stage: &'ir StageInfo<L>) -> Staged<'_, 'ir, Self, L>;
}
```

### Removed from Trait

Two static methods move to free functions (they are dispatch utilities, not interpreter behavior):

- `map_dispatch_miss(stage_id, miss) -> Error`
- `dispatch_in_pipeline(pipeline, stage_id, action) -> Result<R, Error>`

### Rename: EvalCall → CallSemantics

```rust
pub trait CallSemantics<'ir, I: Interpreter<'ir>, L: Dialect>: Dialect {
    type Result;

    fn eval_call(
        &self,
        interpreter: &mut I,
        stage: &'ir StageInfo<L>,
        callee: SpecializedFunction,
        args: &[I::Value],
    ) -> Result<Self::Result, I::Error>;
}
```

Rationale: "CallSemantics" pairs with "Interpretable" — statement semantics via `Interpretable`, call semantics via `CallSemantics`. The old name `EvalCall` was opaque about its role.

`SSACFGRegion` keeps its name (precise technical term) and continues to provide blanket `CallSemantics` impls for standard function bodies.

## Merged Stage Builder

### Before

Two builder types with overlapping methods:
- `InStage<'a, I, L>` — resolves active stage dynamically
- `WithStage<'a, 'ir, I, L>` — takes explicit pre-resolved stage

### After

One builder type: `Staged<'a, 'ir, I, L>`. Always holds a `&'ir StageInfo<L>` — the `in_stage()` constructor resolves it eagerly.

```rust
// From active stage (replaces InStage)
interp.in_stage::<MyDialect>()     → Staged<'_, 'ir, I, MyDialect>

// From explicit stage (replaces WithStage)
interp.with_stage::<L>(stage_info) → Staged<'_, 'ir, I, L>
```

### Methods on Staged (StackInterpreter)

- `step()` — execute current statement
- `advance(control)` — apply cursor mutations
- `call(callee, args)` — call a specialized function
- `run()` — run until Return/Halt/Call
- `run_until_break()` — run until breakpoint

### Methods on Staged (AbstractInterpreter)

- `analyze(callee, args)` — analyze a specialized function
- `summary(callee, args)` — query function summary
- `summary_cache(callee)` — query summary cache
- `insert_summary(callee)` — insert summary (builder pattern)
- `invalidate_summary(callee)` — invalidate summaries
- `remove_summary(callee)` — remove summaries

### Eliminated

All `_in_stage` method variants on `AbstractInterpreter` are removed. Stage-scoped operations go through `Staged`:

```rust
// Before (11 summary methods)
interp.summary(callee, args)
interp.summary_in_stage(stage, callee, args)
interp.summary_cache(callee)
interp.summary_cache_in_stage(stage, callee)
// ... etc

// After (5 on Staged + 1 on AbstractInterpreter)
interp.in_stage::<L>().summary(callee, args)
interp.with_stage::<L>(stage).summary(callee, args)
interp.gc_summaries()  // only this stays on AbstractInterpreter directly
```

## Export Structure

### Tier 1: Prelude (dialect authors)

```rust
pub mod prelude {
    pub use crate::{
        Interpreter,        // trait for bounds
        Interpretable,      // trait to implement
        Continuation,       // return type from interpret()
        InterpreterError,   // error type
        BranchCondition,    // for conditional branches
        CallSemantics,      // for function dialects
        SSACFGRegion,       // for function body types
        Args,               // continuation argument list
    };
}
```

8 symbols. This is all a dialect author needs to learn.

### Tier 2: Root exports (interpreter builders + advanced users)

Everything in prelude, plus:

- `StackInterpreter` — concrete interpreter
- `AbstractInterpreter` — abstract interpreter
- `Staged` — stage-scoped API builder
- `ConcreteExt` — Break/Halt variants for pattern matching
- `Frame` — call frame type
- `FrameStack` — frame storage

### Tier 3: analysis submodule (abstract interpretation specifics)

```rust
pub mod analysis {
    pub use crate::{
        AbstractValue,      // lattice trait
        WideningStrategy,   // join/widen enum
        FixpointState,      // per-frame fixpoint data
        SummaryCache,       // function summary storage
        SummaryEntry,       // individual summary
        AnalysisResult,     // analysis output
        DedupScheduler,     // worklist utility
    };
}
```

### Removed from Public API

- `ConcreteContinuation` type alias — use `Continuation<V, ConcreteExt>` directly
- `AbstractContinuation` type alias — use `Continuation<V>` directly
- `EvalBlock` trait — folded into `Interpreter`
- `InStage` / `WithStage` — replaced by `Staged`

## Migration Impact

### Dialect Authors

```rust
// Before
use kirin_interpreter::{Interpreter, Interpretable, Continuation, EvalBlock, ...};

impl<'ir, I, L> Interpretable<'ir, I, L> for MyOp
where I: Interpreter<'ir> + EvalBlock<'ir, L> { ... }

// After
use kirin_interpreter::prelude::*;

impl<'ir, I, L> Interpretable<'ir, I, L> for MyOp
where I: Interpreter<'ir> { ... }  // EvalBlock bound gone
```

### Function Dialect Authors

```rust
// Before
use kirin_interpreter::{EvalCall, SSACFGRegion};
impl SSACFGRegion for MyBody { ... }  // gets EvalCall for free

// After
use kirin_interpreter::prelude::*;
impl SSACFGRegion for MyBody { ... }  // gets CallSemantics for free
```

### Interpreter Builders

```rust
// Before: implement Interpreter (14 methods) + EvalBlock separately
// After: implement Interpreter (12 methods, includes eval_block + bind_block_args)
```

### Abstract Interpreter Users

```rust
// Before
interp.summary_in_stage(stage, callee, args)

// After
interp.with_stage::<L>(stage).summary(callee, args)
```

## Summary

| Metric | Before | After |
|--------|--------|-------|
| Traits dialect authors encounter | 5 | 4 |
| Traits for bounds (dialect ops) | 2 (`Interpreter` + `EvalBlock`) | 1 (`Interpreter`) |
| Stage builder types | 2 | 1 |
| Summary methods | 11 | 6 |
| Type aliases to learn | 3 | 0 |
| Prelude symbols | N/A | 8 |
| Root exports | ~25 | ~14 |
