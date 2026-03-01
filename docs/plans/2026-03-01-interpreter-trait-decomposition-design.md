# Interpreter Framework Trait Decomposition

**Date:** 2026-03-01
**Status:** Proposed

## Problem

The `Interpreter<'ir>` trait mixes three concerns (value storage, stage resolution, block evaluation) into 12 methods and 4 associated types. `InterpreterError` has 17 variants, most of which are internal stage-resolution details. Module organization exposes implementation internals (dispatch tables, frame extras) that dialect authors never use.

Three downstream personas need clean APIs:
1. **Dialect authors** — implement `Interpretable`/`CallSemantics` for their ops
2. **Interpreter consumers** — instantiate `StackInterpreter`/`AbstractInterpreter` and run programs
3. **Custom interpreter developers** — build new interpreter types (profiler, tracer) from composable building blocks

## Design

### Trait Decomposition

Split `Interpreter<'ir>` into three focused sub-traits:

**`ValueStore`** — value read/write (3 methods, 2 associated types)
```rust
pub trait ValueStore {
    type Value;
    type Error;
    fn read(&self, value: SSAValue) -> Result<Self::Value, Self::Error>;
    fn write(&mut self, result: ResultValue, value: Self::Value) -> Result<(), Self::Error>;
    fn write_ssa(&mut self, ssa: SSAValue, value: Self::Value) -> Result<(), Self::Error>;
}
```

**`StageAccess<'ir>`** — pipeline and stage resolution (2 required + 5 provided, 1 associated type)
```rust
pub trait StageAccess<'ir>: Sized {
    type StageInfo: StageMeta;
    fn pipeline(&self) -> &'ir Pipeline<Self::StageInfo>;
    fn active_stage(&self) -> CompileStage;
    // Provided: active_stage_info, resolve_stage_id, resolve_stage_info, in_stage, with_stage
}
```

**`BlockEvaluator<'ir>`** — block-level execution (2 required, 1 associated type)
```rust
pub trait BlockEvaluator<'ir>: ValueStore {
    type Ext: Debug;
    fn eval_block<L: Dialect>(&mut self, stage: &'ir StageInfo<L>, block: Block)
        -> Result<Continuation<Self::Value, Self::Ext>, Self::Error>;
    fn bind_block_args<L: Dialect>(&mut self, stage: &'ir StageInfo<L>, block: Block, args: &[Self::Value])
        -> Result<(), Self::Error>;
}
```

**`Interpreter<'ir>`** — blanket supertrait
```rust
pub trait Interpreter<'ir>: ValueStore + StageAccess<'ir> + BlockEvaluator<'ir> {}
impl<'ir, T> Interpreter<'ir> for T where T: ValueStore + StageAccess<'ir> + BlockEvaluator<'ir> {}
```

Dialect authors continue writing `I: Interpreter<'ir>`. Custom interpreter developers implement sub-traits independently.

### InterpreterError Simplification (9 variants, down from 17)

```rust
pub enum InterpreterError {
    NoFrame,
    UnboundValue(SSAValue),
    FuelExhausted,
    MaxDepthExceeded,
    MissingEntry,
    ArityMismatch { expected: usize, got: usize },
    StageResolution { stage: CompileStage, kind: StageResolutionError },
    UnexpectedControl(String),
    Custom(Box<dyn Error + Send + Sync>),
}

pub enum StageResolutionError {
    MissingStage,
    MissingDialect,
    TypeMismatch,
    MissingFunction { function: SpecializedFunction },
    UnknownTarget { name: String },
    NoSpecialization { staged_function: ... },
    AmbiguousSpecialization { count: usize },
    MissingCallee { callee: SpecializedFunction },
}
```

Stage/pipeline resolution details are nested in `StageResolutionError`. `UnexpectedControl` + `UnsupportedForkAction` merge into one variant.

### Module Organization

```
kirin-interpreter/src/
├── lib.rs              → Crate root re-exports
├── value.rs            → ValueStore, BranchCondition
├── stage.rs            → StageAccess, Staged
├── block.rs            → BlockEvaluator
├── interpreter.rs      → Interpreter supertrait (blanket impl)
├── interpretable.rs    → Interpretable trait
├── call.rs             → CallSemantics, SSACFGRegion
├── control.rs          → Continuation, ConcreteExt, Args
├── error.rs            → InterpreterError, StageResolutionError
├── dispatch.rs         → DispatchCache, dispatch_in_pipeline, map_dispatch_miss
├── frame.rs            → Frame<V, X>
├── frame_stack.rs      → FrameStack<V, X>
├── result.rs           → AnalysisResult<V>
├── stack/              → StackInterpreter (dispatch internals private)
├── abstract_interp/    → AbstractInterpreter (fixpoint internals private)
├── scheduler.rs        → DedupScheduler
├── value_domain.rs     → AbstractValue
└── widening.rs         → WideningStrategy
```

### Public Dispatch Module

```rust
pub mod dispatch {
    /// Cached per-stage dispatch results. O(1) lookup by stage index.
    pub struct DispatchCache<Entry> { ... }
    impl<Entry> DispatchCache<Entry> {
        pub fn build<S, A, E>(pipeline, resolve) -> Self;
        pub fn get(&self, stage: CompileStage) -> Option<&Entry>;
    }
    /// Route a runtime action to the correct dialect handler.
    pub fn dispatch_in_pipeline<S, A, R, E>(...) -> Result<R, E>;
    /// Convert a stage dispatch miss into InterpreterError.
    pub fn map_dispatch_miss<E>(...) -> E;
}
```

StackInterpreter uses `DispatchCache<DynFrameDispatch>` internally. Custom interpreters use `DispatchCache<TheirEntry>` for their own dispatch tables.

### Visibility Changes

**Newly private:** `DynFrameDispatch`, `DynStepFn`, `DynAdvanceFn`, `DynPushCallFrameFn`, `StackFrameExtra`, `StackFrame` type alias, `StageDispatchTable`.

**Stay public (building blocks):** `Frame<V,X>`, `FrameStack<V,X>`, `DispatchCache<E>`, `DedupScheduler<W>`, `FixpointState`, `SummaryCache<V>`, `SummaryEntry<V>`.

### Prelude and Analysis

**Prelude (7 symbols, for dialect authors):**
`Continuation`, `Interpretable`, `Interpreter`, `InterpreterError`, `BranchCondition`, `CallSemantics`, `SSACFGRegion`

(`Args` removed — only used by kirin-function's `Call` op.)

**Analysis (7 symbols, unchanged):**
`AbstractValue`, `AnalysisResult`, `DedupScheduler`, `FixpointState`, `SummaryCache`, `SummaryEntry`, `WideningStrategy`

## Impact on Existing Code

Dialect authors: no meaningful change. `I: Interpreter<'ir>` provides the same methods.

Custom interpreter developers gain: composable sub-traits, reusable `Frame`/`FrameStack`/`DispatchCache` building blocks, cleaner error model.
