# Interpreter Trait Decomposition Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Decompose the monolithic `Interpreter<'ir>` trait into `ValueStore`, `StageAccess<'ir>`, `BlockEvaluator<'ir>`, with `Interpreter<'ir>` as a blanket supertrait. Simplify `InterpreterError` from 17 to 9 variants. Add public `DispatchCache`. Reorganize modules.

**Architecture:** Bottom-up refactor — introduce new traits first, migrate the two interpreter impls, then update all downstream consumers (7 dialect crates, 4 test files). The `Interpreter<'ir>` blanket supertrait ensures dialect authors see no breaking changes in their `I: Interpreter<'ir>` bounds.

**Tech Stack:** Rust 2024 edition, thiserror for errors, kirin-ir types (SSAValue, ResultValue, CompileStage, Pipeline, StageInfo, StageMeta, Dialect, etc.)

---

### Task 1: Introduce `StageResolutionError` and simplify `InterpreterError`

This is the lowest-risk change — it touches only `error.rs` and the sites that construct error variants.

**Files:**
- Modify: `crates/kirin-interpreter/src/error.rs`
- Modify: `crates/kirin-interpreter/src/interpreter.rs:80-89` (resolve_stage_info error construction)
- Modify: `crates/kirin-interpreter/src/dispatch.rs:10-16` (map_dispatch_miss)
- Modify: `crates/kirin-interpreter/src/stack/call.rs` (all InterpreterError construction sites)
- Modify: `crates/kirin-interpreter/src/abstract_interp/fixpoint.rs` (error construction sites)

**Step 1: Write failing tests**

Add to a new test file `crates/kirin-interpreter/tests/error_variants.rs`:

```rust
use kirin_interpreter::InterpreterError;

#[test]
fn stage_resolution_error_display() {
    let err = InterpreterError::StageResolution {
        stage: kirin_ir::CompileStage::default(),
        kind: kirin_interpreter::StageResolutionError::MissingStage,
    };
    let msg = format!("{err}");
    assert!(msg.contains("stage"), "error message should mention stage: {msg}");
}

#[test]
fn unexpected_control_merges_fork() {
    let err = InterpreterError::UnexpectedControl("fork not supported".into());
    let msg = format!("{err}");
    assert!(msg.contains("fork"), "should contain reason: {msg}");
}
```

**Step 2: Run test to verify it fails**

Run: `cargo nextest run -p kirin-interpreter -E 'test(error_variants)'`
Expected: Compile error — `StageResolutionError` does not exist yet.

**Step 3: Implement the error refactor**

Replace `crates/kirin-interpreter/src/error.rs` with:

```rust
use kirin_ir::{CompileStage, SSAValue, SpecializedFunction, StagedFunction, Function};

/// Detailed reason for a stage/pipeline resolution failure.
#[derive(Debug, thiserror::Error)]
pub enum StageResolutionError {
    /// The requested stage does not exist in the pipeline.
    #[error("missing compile stage")]
    MissingStage,
    /// The stage exists but does not contain the requested dialect.
    #[error("stage does not contain the requested dialect")]
    MissingDialect,
    /// Typed API was called with a dialect not present in the current frame stage.
    #[error("typed API stage mismatch: dialect not present")]
    TypeMismatch,
    /// Function has no staged-function mapping for the requested stage.
    #[error("function {function:?} has no staged function mapping")]
    MissingFunction { function: Function },
    /// No abstract function with the requested symbolic name exists.
    #[error("unknown function target '{name}'")]
    UnknownTarget { name: String },
    /// No live specialization exists for the requested staged function/stage pair.
    #[error("no live specialization for {staged_function:?}")]
    NoSpecialization { staged_function: StagedFunction },
    /// More than one live specialization exists when unique-or-error is required.
    #[error("ambiguous: {count} live specializations for {staged_function:?}")]
    AmbiguousSpecialization {
        staged_function: StagedFunction,
        count: usize,
    },
    /// A continuation referred to a callee that does not exist at the given stage.
    #[error("callee {callee:?} is not defined")]
    MissingCallee { callee: SpecializedFunction },
}

/// Error type for interpreter failures.
///
/// Framework errors cover common interpreter failure modes. User-defined
/// errors (e.g. division by zero, type errors) go in the [`Custom`](Self::Custom)
/// variant via [`InterpreterError::custom`].
#[derive(Debug, thiserror::Error)]
pub enum InterpreterError {
    /// No call frame on the stack.
    #[error("no active call frame")]
    NoFrame,
    /// An SSA value was read before being written.
    #[error("unbound SSA value: {0:?}")]
    UnboundValue(SSAValue),
    /// Step fuel has been exhausted.
    #[error("step fuel exhausted")]
    FuelExhausted,
    /// Call depth exceeded the configured maximum.
    #[error("call depth exceeded maximum")]
    MaxDepthExceeded,
    /// Function entry block could not be resolved.
    #[error("function entry block not found")]
    MissingEntry,
    /// Argument count does not match block/function parameter count.
    #[error("arity mismatch: expected {expected} arguments, got {got}")]
    ArityMismatch { expected: usize, got: usize },
    /// A stage or pipeline resolution failed.
    #[error("stage resolution error at {stage:?}: {kind}")]
    StageResolution {
        stage: CompileStage,
        kind: StageResolutionError,
    },
    /// An unexpected control flow action was encountered.
    #[error("unexpected control flow: {0}")]
    UnexpectedControl(String),
    /// User-defined error.
    #[error(transparent)]
    Custom(Box<dyn std::error::Error + Send + Sync>),
}

impl InterpreterError {
    /// Wrap an arbitrary error as [`InterpreterError::Custom`].
    pub fn custom(error: impl std::error::Error + Send + Sync + 'static) -> Self {
        InterpreterError::Custom(Box::new(error))
    }
}
```

Then update every call site that constructs the old variants. The key mappings:

| Old variant | New variant |
|-------------|-------------|
| `MissingStage { stage }` | `StageResolution { stage, kind: StageResolutionError::MissingStage }` |
| `MissingStageDialect { stage }` | `StageResolution { stage, kind: StageResolutionError::MissingDialect }` |
| `TypedStageMismatch { frame_stage }` | `StageResolution { stage: frame_stage, kind: StageResolutionError::TypeMismatch }` |
| `MissingFunctionStageMapping { function, stage }` | `StageResolution { stage, kind: StageResolutionError::MissingFunction { function } }` |
| `UnknownFunctionTarget { name, stage }` | `StageResolution { stage, kind: StageResolutionError::UnknownTarget { name } }` |
| `NoSpecializationAtStage { staged_function, stage }` | `StageResolution { stage, kind: StageResolutionError::NoSpecialization { staged_function } }` |
| `AmbiguousSpecializationAtStage { staged_function, stage, count }` | `StageResolution { stage, kind: StageResolutionError::AmbiguousSpecialization { staged_function, count } }` |
| `MissingCalleeAtStage { callee, stage }` | `StageResolution { stage, kind: StageResolutionError::MissingCallee { callee } }` |
| `UnsupportedForkAction { action }` | `UnexpectedControl(format!("unsupported fork action: {action}"))` |

Search for every old variant name across the crate with: `rg "MissingStage\b|MissingStageDialect|TypedStageMismatch|MissingFunctionStageMapping|UnknownFunctionTarget|NoSpecializationAtStage|AmbiguousSpecializationAtStage|MissingCalleeAtStage|UnsupportedForkAction" crates/kirin-interpreter/src/`

Update each construction site accordingly.

**Step 4: Re-export `StageResolutionError` from lib.rs**

In `crates/kirin-interpreter/src/lib.rs`, change:
```rust
pub use error::InterpreterError;
```
to:
```rust
pub use error::{InterpreterError, StageResolutionError};
```

**Step 5: Run all tests**

Run: `cargo nextest run -p kirin-interpreter`
Run: `cargo nextest run --workspace`
Expected: All pass. No downstream crates match on the internal variants directly — they only use `From<InterpreterError>`.

**Step 6: Commit**

```bash
git add -A crates/kirin-interpreter/
git commit -m "refactor(interpreter): simplify InterpreterError to 9 variants with nested StageResolutionError"
```

---

### Task 2: Extract `ValueStore` trait

**Files:**
- Create: `crates/kirin-interpreter/src/value_store.rs`
- Modify: `crates/kirin-interpreter/src/interpreter.rs` — remove read/write/write_ssa from Interpreter, add ValueStore supertrait
- Modify: `crates/kirin-interpreter/src/lib.rs` — add module and re-export
- Modify: `crates/kirin-interpreter/src/stack/frame.rs:100-148` — split Interpreter impl into ValueStore impl + Interpreter impl
- Modify: `crates/kirin-interpreter/src/abstract_interp/interp.rs:264-334` — same split

**Step 1: Create `value_store.rs`**

Create `crates/kirin-interpreter/src/value_store.rs`:

```rust
use kirin_ir::{ResultValue, SSAValue};

/// Value read/write operations for SSA bindings.
///
/// This is the minimal storage interface that all interpreter implementations
/// share. Dialect `Interpretable` impls use this to read operands and write
/// results.
pub trait ValueStore {
    /// The value type manipulated by this interpreter.
    ///
    /// Values should be cheap to clone — typically pointer-sized handles,
    /// small enums, or wrappers around `Arc`/`Rc` for heavier data.
    type Value: Clone;
    type Error;

    /// Returns a cloned copy of the bound value.
    fn read(&self, value: SSAValue) -> Result<Self::Value, Self::Error>;

    /// Bind a result to a value.
    fn write(&mut self, result: ResultValue, value: Self::Value) -> Result<(), Self::Error>;

    /// Bind an SSA value directly (e.g. block arguments).
    fn write_ssa(&mut self, ssa: SSAValue, value: Self::Value) -> Result<(), Self::Error>;
}
```

**Step 2: Update `interpreter.rs`**

Remove `read`, `write`, `write_ssa`, `Value`, and `Error` from the `Interpreter` trait. Add `ValueStore` as a supertrait:

```rust
pub trait Interpreter<'ir>: ValueStore + Sized + 'ir {
    type Ext: fmt::Debug;
    type StageInfo: StageMeta;

    fn pipeline(&self) -> &'ir Pipeline<Self::StageInfo>;
    fn active_stage(&self) -> CompileStage;

    // ... keep all provided and remaining required methods ...
    // bind_block_args, eval_block, in_stage, with_stage, active_stage_info,
    // resolve_stage_id, resolve_stage_info
}
```

Note: `bind_block_args` uses `Self::Value` from `ValueStore`, and `eval_block` returns `Continuation<Self::Value, Self::Ext>` — both compile because `ValueStore` is a supertrait.

**Step 3: Split StackInterpreter impl**

In `crates/kirin-interpreter/src/stack/frame.rs`, split the single `impl Interpreter<'ir>` block (lines 100-148) into two impls:

```rust
impl<'ir, V, S, E, G> ValueStore for StackInterpreter<'ir, V, S, E, G>
where
    V: Clone + 'ir,
    E: From<InterpreterError> + 'ir,
    S: StageMeta + 'ir,
    G: 'ir,
{
    type Value = V;
    type Error = E;

    fn read(&self, value: SSAValue) -> Result<V, E> { self.frames.read(value).cloned() }
    fn write(&mut self, result: ResultValue, value: V) -> Result<(), E> { self.frames.write(result, value) }
    fn write_ssa(&mut self, ssa: SSAValue, value: V) -> Result<(), E> { self.frames.write_ssa(ssa, value) }
}

impl<'ir, V, S, E, G> Interpreter<'ir> for StackInterpreter<'ir, V, S, E, G>
where
    V: Clone + 'ir,
    E: From<InterpreterError> + 'ir,
    S: StageMeta + 'ir,
    G: 'ir,
{
    type Ext = ConcreteExt;
    type StageInfo = S;

    fn pipeline(&self) -> &'ir Pipeline<S> { self.pipeline }
    fn active_stage(&self) -> CompileStage { self.frames.active_stage_or(self.root_stage) }
    fn eval_block<L: Dialect>(...) -> ... { /* unchanged */ }
}
```

**Step 4: Split AbstractInterpreter impl**

Same pattern in `crates/kirin-interpreter/src/abstract_interp/interp.rs` (lines 264-334).

**Step 5: Update lib.rs**

```rust
mod value_store;
pub use value_store::ValueStore;
```

Add `ValueStore` to the prelude:
```rust
pub mod prelude {
    pub use crate::{
        BranchCondition, CallSemantics, Continuation, Interpretable, Interpreter,
        InterpreterError, SSACFGRegion, ValueStore,
    };
}
```

**Step 6: Run tests**

Run: `cargo nextest run --workspace`
Expected: All pass. Dialect authors use `I: Interpreter<'ir>` which now implies `ValueStore`, so `I::Value`, `interp.read()`, `interp.write()` still resolve.

**Step 7: Commit**

```bash
git add crates/kirin-interpreter/src/
git commit -m "refactor(interpreter): extract ValueStore trait from Interpreter"
```

---

### Task 3: Extract `StageAccess<'ir>` trait

**Files:**
- Create: `crates/kirin-interpreter/src/stage_access.rs`
- Modify: `crates/kirin-interpreter/src/interpreter.rs` — remove pipeline/active_stage/provided methods, add StageAccess supertrait
- Modify: `crates/kirin-interpreter/src/stage.rs` — update Staged to use StageAccess bound
- Modify: `crates/kirin-interpreter/src/stack/frame.rs` — add StageAccess impl
- Modify: `crates/kirin-interpreter/src/abstract_interp/interp.rs` — add StageAccess impl
- Modify: `crates/kirin-interpreter/src/lib.rs` — add module and re-export

**Step 1: Create `stage_access.rs`**

Create `crates/kirin-interpreter/src/stage_access.rs`:

```rust
use kirin_ir::{CompileStage, Dialect, HasStageInfo, Pipeline, StageInfo, StageMeta};

use crate::InterpreterError;
use crate::stage::Staged;
use crate::StageResolutionError;

/// Pipeline and stage resolution for interpreter implementations.
///
/// Provides access to the IR pipeline and active compilation stage, plus
/// provided methods for dialect-typed stage resolution.
pub trait StageAccess<'ir>: Sized {
    type StageInfo: StageMeta;

    /// Reference to the IR pipeline.
    fn pipeline(&self) -> &'ir Pipeline<Self::StageInfo>;

    /// The currently active compilation stage.
    fn active_stage(&self) -> CompileStage;

    /// Resolve the [`StageInfo`] for dialect `L` from the active stage.
    ///
    /// # Panics
    ///
    /// Panics if the active stage does not contain a `StageInfo<L>`.
    fn active_stage_info<L>(&self) -> &'ir StageInfo<L>
    where
        Self::StageInfo: HasStageInfo<L>,
        L: Dialect,
    {
        self.pipeline()
            .stage(self.active_stage())
            .and_then(|s| s.try_stage_info())
            .expect("active stage does not contain StageInfo for this dialect")
    }

    /// Returns the stage ID from `stage`, falling back to the active stage
    /// if the stage info is not attached to a pipeline stage.
    fn resolve_stage_id<L: Dialect>(&self, stage: &StageInfo<L>) -> CompileStage {
        stage.stage_id().unwrap_or_else(|| self.active_stage())
    }

    /// Resolve a stage-specific dialect view for `stage_id` with explicit
    /// errors instead of panicking.
    fn resolve_stage_info<L>(
        &self,
        stage_id: CompileStage,
    ) -> Result<&'ir StageInfo<L>, InterpreterError>
    where
        Self::StageInfo: HasStageInfo<L>,
        L: Dialect,
    {
        let stage = self
            .pipeline()
            .stage(stage_id)
            .ok_or_else(|| InterpreterError::StageResolution {
                stage: stage_id,
                kind: StageResolutionError::MissingStage,
            })?;
        <Self::StageInfo as HasStageInfo<L>>::try_stage_info(stage).ok_or_else(|| {
            InterpreterError::StageResolution {
                stage: stage_id,
                kind: StageResolutionError::TypeMismatch,
            }
        })
    }

    /// Resolve typed-stage APIs from the current active stage.
    fn in_stage<L>(&mut self) -> Staged<'_, 'ir, Self, L>
    where
        Self::StageInfo: HasStageInfo<L>,
        L: Dialect,
    {
        let stage = self.active_stage_info::<L>();
        Staged {
            interp: self,
            stage,
        }
    }

    /// Bind APIs to an explicit stage reference.
    fn with_stage<L: Dialect>(&mut self, stage: &'ir StageInfo<L>) -> Staged<'_, 'ir, Self, L> {
        Staged {
            interp: self,
            stage,
        }
    }
}
```

**Important note on `resolve_stage_info` return type:** The old trait returned `Result<_, Self::Error>` which required `Self::Error: From<InterpreterError>`. The new trait returns `Result<_, InterpreterError>` directly, since stage resolution is a framework concern. Call sites that need `Self::Error` can use `?` with `From<InterpreterError>` on their error type. Check every call to `resolve_stage_info` in the crate and ensure the `?` operator still works (it should, since `E: From<InterpreterError>` is already required everywhere).

**Step 2: Slim down `interpreter.rs`**

Remove `pipeline`, `active_stage`, `active_stage_info`, `resolve_stage_id`, `resolve_stage_info`, `in_stage`, `with_stage` from the Interpreter trait. Add `StageAccess` as a supertrait:

```rust
use crate::ValueStore;
use crate::stage_access::StageAccess;

pub trait Interpreter<'ir>: ValueStore + StageAccess<'ir> + Sized + 'ir {
    type Ext: fmt::Debug;

    fn bind_block_args<L: Dialect>(
        &mut self,
        stage: &'ir StageInfo<L>,
        block: Block,
        args: &[Self::Value],
    ) -> Result<(), Self::Error>
    where
        Self::Error: From<InterpreterError>,
    { /* same default impl */ }

    fn eval_block<L: Dialect>(
        &mut self,
        stage: &'ir StageInfo<L>,
        block: Block,
    ) -> Result<Continuation<Self::Value, Self::Ext>, Self::Error>
    where
        Self::StageInfo: HasStageInfo<L>,
        L: crate::Interpretable<'ir, Self, L>;
}
```

Note: `StageInfo` associated type is now on `StageAccess`, not duplicated on `Interpreter`.

**Step 3: Add StageAccess impls for both interpreters**

In `crates/kirin-interpreter/src/stack/frame.rs`:
```rust
impl<'ir, V, S, E, G> StageAccess<'ir> for StackInterpreter<'ir, V, S, E, G>
where
    V: Clone + 'ir,
    E: From<InterpreterError> + 'ir,
    S: StageMeta + 'ir,
    G: 'ir,
{
    type StageInfo = S;
    fn pipeline(&self) -> &'ir Pipeline<S> { self.pipeline }
    fn active_stage(&self) -> CompileStage { self.frames.active_stage_or(self.root_stage) }
}
```

Remove `pipeline`, `active_stage`, `StageInfo` from the `Interpreter` impl block.

Same pattern for `AbstractInterpreter` in `crates/kirin-interpreter/src/abstract_interp/interp.rs`.

**Step 4: Update `stage.rs`**

The `Staged` struct needs `StageAccess` bound:
```rust
pub struct Staged<'a, 'ir, I: StageAccess<'ir>, L: Dialect> {
    pub(crate) interp: &'a mut I,
    pub(crate) stage: &'ir StageInfo<L>,
}
```

Update `expect_stage_id` visibility if needed (it stays `pub(crate)`).

**Step 5: Update lib.rs**

```rust
mod stage_access;
pub use stage_access::StageAccess;
```

Add to prelude:
```rust
pub mod prelude {
    pub use crate::{
        BranchCondition, CallSemantics, Continuation, Interpretable, Interpreter,
        InterpreterError, SSACFGRegion, StageAccess, ValueStore,
    };
}
```

**Step 6: Fix internal call sites**

Search for `self.pipeline()`, `self.active_stage()`, `self.resolve_stage_info()`, `self.active_stage_info()`, `self.in_stage()`, `self.with_stage()` inside the kirin-interpreter crate. These should all still resolve via the `StageAccess` supertrait. The key places:

- `crates/kirin-interpreter/src/stack/transition.rs` — uses `self.pipeline()`, `self.resolve_stage_info()`
- `crates/kirin-interpreter/src/stack/call.rs` — uses `self.resolve_stage_info()`, `self.in_stage()`, `self.with_stage()`
- `crates/kirin-interpreter/src/stack/stage.rs` — `Staged` methods
- `crates/kirin-interpreter/src/abstract_interp/fixpoint.rs` — uses `self.resolve_stage_id()`, `self.resolve_stage_info()`

Make sure all `use crate::Interpreter;` imports in internal files also add `use crate::StageAccess;` where stage methods are called directly on the interpreter.

**Step 7: Run tests**

Run: `cargo nextest run --workspace`
Expected: All pass.

**Step 8: Commit**

```bash
git add crates/kirin-interpreter/src/
git commit -m "refactor(interpreter): extract StageAccess trait from Interpreter"
```

---

### Task 4: Extract `BlockEvaluator<'ir>` trait

**Files:**
- Create: `crates/kirin-interpreter/src/block_eval.rs`
- Modify: `crates/kirin-interpreter/src/interpreter.rs` — remove bind_block_args/eval_block, add BlockEvaluator supertrait
- Modify: `crates/kirin-interpreter/src/stack/frame.rs` — add BlockEvaluator impl
- Modify: `crates/kirin-interpreter/src/abstract_interp/interp.rs` — add BlockEvaluator impl
- Modify: `crates/kirin-interpreter/src/lib.rs` — add module and re-export

**Step 1: Create `block_eval.rs`**

Create `crates/kirin-interpreter/src/block_eval.rs`:

```rust
use std::fmt;

use kirin_ir::{Block, Dialect, HasStageInfo, SSAValue, StageInfo};

use crate::{Continuation, InterpreterError, ValueStore};

/// Block-level execution contract.
///
/// This is where `StackInterpreter` and `AbstractInterpreter` fundamentally
/// diverge: the former uses cursor-based execution, the latter uses
/// statement-by-statement interpretation with worklist propagation.
pub trait BlockEvaluator<'ir>: ValueStore {
    /// Extra continuation variants for this interpreter.
    ///
    /// Concrete interpreters use [`crate::ConcreteExt`] (Break, Halt).
    /// Abstract interpreters use [`std::convert::Infallible`].
    type Ext: fmt::Debug;

    /// Bind values to a block's arguments in the current frame.
    ///
    /// Resolves the block's argument SSA values from stage info and writes
    /// each provided value. Returns `ArityMismatch` if `args.len()` differs
    /// from the block's declared argument count.
    fn bind_block_args<L: Dialect>(
        &mut self,
        stage: &'ir StageInfo<L>,
        block: Block,
        args: &[Self::Value],
    ) -> Result<(), Self::Error>
    where
        Self::Error: From<InterpreterError>,
    {
        let block_info = block.expect_info(stage);
        if block_info.arguments.len() != args.len() {
            return Err(InterpreterError::ArityMismatch {
                expected: block_info.arguments.len(),
                got: args.len(),
            }
            .into());
        }
        let arg_ssas: Vec<SSAValue> = block_info
            .arguments
            .iter()
            .map(|ba| SSAValue::from(*ba))
            .collect();
        for (ssa, val) in arg_ssas.iter().zip(args.iter()) {
            self.write_ssa(*ssa, val.clone())?;
        }
        Ok(())
    }

    /// Execute a body block whose arguments have already been bound.
    ///
    /// Returns a [`Continuation`] representing the block's result.
    fn eval_block<L: Dialect>(
        &mut self,
        stage: &'ir StageInfo<L>,
        block: Block,
    ) -> Result<Continuation<Self::Value, Self::Ext>, Self::Error>
    where
        Self::StageInfo: HasStageInfo<L>,
        L: crate::Interpretable<'ir, Self, L>;
}
```

**Important:** `bind_block_args` has a default implementation (it was already provided in the old `Interpreter` trait). `eval_block` is required. Note `Self::StageInfo` — this comes from `StageAccess`, but `BlockEvaluator` only requires `ValueStore` as a supertrait. We need to either:
- (a) Make `BlockEvaluator` also require `StageAccess` as a supertrait, OR
- (b) Remove the `Self::StageInfo: HasStageInfo<L>` bound from `eval_block`

Option (a) is cleaner since `eval_block` genuinely needs stage access. Update the trait:

```rust
pub trait BlockEvaluator<'ir>: ValueStore + StageAccess<'ir> {
    type Ext: fmt::Debug;
    // ...
}
```

This means `BlockEvaluator` requires both `ValueStore` and `StageAccess`. The `Interpreter` supertrait then just adds `BlockEvaluator`.

**Step 2: Slim down `interpreter.rs` to blanket supertrait**

```rust
use crate::{BlockEvaluator, StageAccess, ValueStore};

/// Unified interpreter trait — automatically implemented for any type that
/// implements [`ValueStore`], [`StageAccess`], and [`BlockEvaluator`].
///
/// Dialect authors should use `I: Interpreter<'ir>` in their trait bounds.
/// Custom interpreter developers implement the sub-traits individually.
pub trait Interpreter<'ir>: BlockEvaluator<'ir> {}

impl<'ir, T> Interpreter<'ir> for T where T: BlockEvaluator<'ir> {}
```

Since `BlockEvaluator<'ir>: ValueStore + StageAccess<'ir>`, `Interpreter<'ir>` transitively requires all three. The blanket impl means anything implementing `BlockEvaluator<'ir>` (which requires the other two) automatically gets `Interpreter<'ir>`.

**Step 3: Add BlockEvaluator impls**

For StackInterpreter in `stack/frame.rs`:
```rust
impl<'ir, V, S, E, G> BlockEvaluator<'ir> for StackInterpreter<'ir, V, S, E, G>
where
    V: Clone + 'ir,
    E: From<InterpreterError> + 'ir,
    S: StageMeta + 'ir,
    G: 'ir,
{
    type Ext = ConcreteExt;

    fn eval_block<L: Dialect>(
        &mut self,
        stage: &'ir StageInfo<L>,
        block: Block,
    ) -> Result<Continuation<V, ConcreteExt>, E>
    where
        S: HasStageInfo<L>,
        L: Interpretable<'ir, Self, L>,
    {
        // ... same implementation ...
    }
}
```

Remove the old `Interpreter` impl block entirely — it's now auto-derived.

Same for AbstractInterpreter.

**Step 4: Update lib.rs**

```rust
mod block_eval;
pub use block_eval::BlockEvaluator;
```

**Step 5: Fix internal references**

Search the crate for `use crate::Interpreter` and ensure `BlockEvaluator` is also imported where `eval_block` or `bind_block_args` are called. Key files:
- `crates/kirin-interpreter/src/eval/call.rs` — blanket impls for SSACFGRegion
- `crates/kirin-interpreter/src/stack/transition.rs` — `advance_frame_with_stage` calls `bind_block_args`
- `crates/kirin-interpreter/src/abstract_interp/fixpoint.rs` — calls `eval_block`

In these files, the `I: Interpreter<'ir>` bound already implies `BlockEvaluator<'ir>`, so `self.eval_block()` and `self.bind_block_args()` resolve. But if methods are called on `self` directly (not via a generic `I`), the `use` import matters. Add `use crate::BlockEvaluator;` wherever needed.

**Step 6: Run tests**

Run: `cargo nextest run --workspace`
Expected: All pass.

**Step 7: Commit**

```bash
git add crates/kirin-interpreter/src/
git commit -m "refactor(interpreter): extract BlockEvaluator trait, Interpreter is now a blanket supertrait"
```

---

### Task 5: Extract `DispatchCache` and make dispatch module public

**Files:**
- Modify: `crates/kirin-interpreter/src/dispatch.rs` — add DispatchCache, make functions public
- Modify: `crates/kirin-interpreter/src/stack/interp.rs` — replace StageDispatchTable with DispatchCache
- Modify: `crates/kirin-interpreter/src/stack/frame.rs` — update build_dispatch_table to use DispatchCache
- Modify: `crates/kirin-interpreter/src/stack/transition.rs` — update dispatch lookups
- Modify: `crates/kirin-interpreter/src/lib.rs` — make dispatch module public

**Step 1: Write failing test**

Add to `crates/kirin-interpreter/tests/dispatch_cache.rs`:

```rust
use kirin_interpreter::dispatch::DispatchCache;

#[test]
fn dispatch_cache_build_and_lookup() {
    // This test just verifies the type exists and the API works.
    // We can't easily construct a Pipeline in a unit test,
    // so we test the empty-cache path.
    let cache: DispatchCache<i32> = DispatchCache::empty();
    assert!(cache.is_empty());
}
```

**Step 2: Run test to verify it fails**

Run: `cargo nextest run -p kirin-interpreter -E 'test(dispatch_cache)'`
Expected: Compile error — `DispatchCache` does not exist.

**Step 3: Implement DispatchCache**

Rewrite `crates/kirin-interpreter/src/dispatch.rs`:

```rust
use kirin_ir::{CompileStage, Pipeline, StageDispatchMiss, StageMeta, SupportsStageDispatch};

use crate::{InterpreterError, StageResolutionError};

/// Cached per-stage dispatch results.
///
/// Pre-computes one entry per stage at construction time. Runtime lookups are
/// O(1) by stage index. Custom interpreter developers can use this with their
/// own entry types.
pub struct DispatchCache<Entry> {
    by_stage: Vec<Option<Entry>>,
}

impl<Entry> DispatchCache<Entry> {
    /// Create an empty cache (no stages).
    pub fn empty() -> Self {
        Self {
            by_stage: Vec::new(),
        }
    }

    /// Returns `true` when the cache has no entries.
    pub fn is_empty(&self) -> bool {
        self.by_stage.is_empty()
    }

    /// Build a dispatch cache by resolving each stage in the pipeline.
    ///
    /// Calls `resolve(pipeline, stage_id)` for each stage that has an ID.
    /// Stages where resolution fails are stored as `None`.
    pub fn build<S, E>(
        pipeline: &Pipeline<S>,
        mut resolve: impl FnMut(&Pipeline<S>, CompileStage) -> Result<Entry, E>,
    ) -> Self
    where
        S: StageMeta,
    {
        let mut by_stage = Vec::with_capacity(pipeline.stages().len());
        for stage in pipeline.stages() {
            let entry = stage
                .stage_id()
                .and_then(|stage_id| resolve(pipeline, stage_id).ok());
            by_stage.push(entry);
        }
        Self { by_stage }
    }

    /// Look up a cached entry by stage index.
    pub fn get(&self, stage: CompileStage) -> Option<&Entry> {
        let idx = stage.index();
        self.by_stage.get(idx).and_then(|e| e.as_ref())
    }
}

/// Convert a stage-dispatch miss into the framework error model.
pub fn map_dispatch_miss<E: From<InterpreterError>>(
    stage_id: CompileStage,
    miss: StageDispatchMiss,
) -> E {
    let kind = match miss {
        StageDispatchMiss::MissingStage => StageResolutionError::MissingStage,
        StageDispatchMiss::MissingDialect => StageResolutionError::MissingDialect,
    };
    InterpreterError::StageResolution {
        stage: stage_id,
        kind,
    }
    .into()
}

/// Dispatch a runtime action against `stage_id` using `pipeline`.
pub fn dispatch_in_pipeline<S, A, R, E>(
    pipeline: &Pipeline<S>,
    stage_id: CompileStage,
    action: &mut A,
) -> Result<R, E>
where
    S: StageMeta + SupportsStageDispatch<A, R, E>,
    E: From<InterpreterError>,
{
    pipeline.dispatch_stage_or_else(stage_id, action, |miss| map_dispatch_miss(stage_id, miss))
}
```

**Step 4: Update StackInterpreter to use DispatchCache**

In `crates/kirin-interpreter/src/stack/interp.rs`, replace:
```rust
pub(super) struct StageDispatchTable<'ir, V, S, E, G> where S: StageMeta {
    pub(super) by_stage: Vec<Option<DynFrameDispatch<'ir, V, S, E, G>>>,
}
```
with:
```rust
use crate::dispatch::DispatchCache;
```

And change the `dispatch_table` field:
```rust
pub(super) dispatch_table: DispatchCache<DynFrameDispatch<'ir, V, S, E, G>>,
```

In `crates/kirin-interpreter/src/stack/frame.rs`, update `build_dispatch_table`:
```rust
pub(super) fn build_dispatch_table(
    pipeline: &'ir Pipeline<S>,
) -> DispatchCache<DynFrameDispatch<'ir, V, S, E, G>>
where /* same bounds */
{
    DispatchCache::build(pipeline, |pipeline, stage_id| {
        Self::resolve_dispatch_for_stage_in_pipeline(pipeline, stage_id)
    })
}
```

In `crates/kirin-interpreter/src/stack/transition.rs`, update lookups from `self.dispatch_table.by_stage.get(idx).copied().flatten()` to `self.dispatch_table.get(stage)`. Check if `CompileStage` has an `index()` method or if `DispatchCache::get` needs to handle the index extraction internally. Look at how `CompileStage` is currently used in the `by_stage` Vec.

**Step 5: Make dispatch module public in lib.rs**

Change:
```rust
mod dispatch;
```
to:
```rust
pub mod dispatch;
```

Remove any re-export of dispatch items at the crate root (they live in the `dispatch` module now).

**Step 6: Run tests**

Run: `cargo nextest run -p kirin-interpreter`
Run: `cargo nextest run --workspace`
Expected: All pass.

**Step 7: Commit**

```bash
git add crates/kirin-interpreter/
git commit -m "refactor(interpreter): add public DispatchCache, replace internal StageDispatchTable"
```

---

### Task 6: Flatten `eval/` into `call.rs` and reorganize module declarations

**Files:**
- Move: `crates/kirin-interpreter/src/eval/call.rs` → `crates/kirin-interpreter/src/call.rs`
- Delete: `crates/kirin-interpreter/src/eval/mod.rs`
- Modify: `crates/kirin-interpreter/src/lib.rs` — replace `mod eval` with `mod call`
- Optionally rename: `crates/kirin-interpreter/src/value.rs` (contains BranchCondition + AbstractValue)

**Step 1: Move the file**

```bash
cp crates/kirin-interpreter/src/eval/call.rs crates/kirin-interpreter/src/call.rs
rm crates/kirin-interpreter/src/eval/call.rs
rm crates/kirin-interpreter/src/eval/mod.rs
rmdir crates/kirin-interpreter/src/eval/
```

**Step 2: Update lib.rs**

Replace:
```rust
mod eval;
pub use eval::{CallSemantics, SSACFGRegion};
```
With:
```rust
mod call;
pub use call::{CallSemantics, SSACFGRegion};
```

**Step 3: Update internal `crate::` paths in call.rs**

Check `call.rs` for any `super::` references that assumed it was inside `eval/`. Change to `crate::` as needed.

**Step 4: Run tests**

Run: `cargo nextest run --workspace`
Expected: All pass.

**Step 5: Commit**

```bash
git add crates/kirin-interpreter/src/
git commit -m "refactor(interpreter): flatten eval/ module into call.rs"
```

---

### Task 7: Update prelude and public API, remove `Args` from prelude

**Files:**
- Modify: `crates/kirin-interpreter/src/lib.rs` — update prelude and re-exports

**Step 1: Update prelude**

```rust
/// Essentials for dialect authors implementing operational semantics.
pub mod prelude {
    pub use crate::{
        BranchCondition, CallSemantics, Continuation, Interpretable, Interpreter,
        InterpreterError, SSACFGRegion,
    };
}
```

Note: `Args` removed. `ValueStore`, `StageAccess`, `BlockEvaluator` are available at crate root but NOT in prelude — dialect authors use `I: Interpreter<'ir>` which gives them everything transitively.

**Step 2: Verify `Args` usage**

Search for `use kirin_interpreter::Args` or `use kirin_interpreter::prelude::*` followed by `Args` usage. The only dialect that uses `Args` directly is kirin-function (in its `Call` op). Update it to import explicitly:
```rust
use kirin_interpreter::Args;
```

**Step 3: Run tests**

Run: `cargo nextest run --workspace`
Expected: All pass.

**Step 4: Commit**

```bash
git add crates/kirin-interpreter/src/lib.rs crates/kirin-function/
git commit -m "refactor(interpreter): update prelude, remove Args from prelude"
```

---

### Task 8: Update downstream dialect crates

All 7 dialect crates import from `kirin_interpreter` in their `interpret_impl.rs`. Since `Interpreter<'ir>` is now a blanket supertrait that provides all the same methods, **no changes should be needed** to dialect code. But verify each one compiles.

**Files to verify:**
- `crates/kirin-cf/src/interpret_impl.rs`
- `crates/kirin-scf/src/interpret_impl.rs`
- `crates/kirin-arith/src/interpret_impl.rs`
- `crates/kirin-constant/src/interpret_impl.rs`
- `crates/kirin-function/src/interpret_impl.rs`
- `crates/kirin-bitwise/src/interpret_impl.rs`
- `crates/kirin-cmp/src/interpret_impl.rs`

**Step 1: Build all dialect crates**

Run: `cargo build --workspace`
Expected: All compile. If any fail, the issue will be:
- Missing `use` imports for the new trait names (unlikely since they use `I: Interpreter<'ir>` which gives everything)
- `InterpreterError` variant name changes from Task 1

**Step 2: Fix any issues**

If a dialect crate matches on specific `InterpreterError` variants (unlikely — they all use `From<InterpreterError>`), update the match arms.

**Step 3: Run full test suite**

Run: `cargo nextest run --workspace`
Run: `cargo test --doc --workspace`
Expected: All pass.

**Step 4: Commit (if any changes needed)**

```bash
git add crates/
git commit -m "fix: update dialect crates for interpreter trait decomposition"
```

---

### Task 9: Update kirin-derive-interpreter for new trait paths

The derive macros in `kirin-derive-interpreter` generate `Interpretable` and `CallSemantics` impls that reference `Interpreter`. Check if they emit `use` statements or fully-qualified paths that need updating.

**Files:**
- Check: `crates/kirin-derive-interpreter/src/interpretable/emit.rs`
- Check: `crates/kirin-derive-interpreter/src/eval_call/emit.rs`

**Step 1: Inspect generated code**

Read the emit files and check what trait paths the generated code uses. The macros likely emit something like `#interpreter_crate::Interpreter` — if so, since `Interpreter` is still re-exported at the crate root, nothing changes.

**Step 2: Run derive macro tests**

Run: `cargo nextest run -p kirin-interpreter -E 'test(derive)'`
Run: `cargo nextest run -p kirin-interpreter -E 'test(stage_dispatch)'`
Expected: All pass.

**Step 3: Commit (if changes needed)**

```bash
git add crates/kirin-derive-interpreter/
git commit -m "fix(derive-interpreter): update trait paths for decomposed Interpreter"
```

---

### Task 10: Update MEMORY.md and CLAUDE.md conventions

**Files:**
- Modify: `/Users/roger/.claude/projects/-Users-roger-Code-rust-kirin/memory/MEMORY.md`
- Modify: `/Users/roger/Code/rust/kirin/CLAUDE.md` (Interpreter Conventions section)

**Step 1: Update MEMORY.md**

Update the "Interpreter Framework" section to reflect the new trait decomposition:
- `ValueStore` trait: read, write, write_ssa (Value, Error associated types)
- `StageAccess<'ir>` trait: pipeline, active_stage + provided stage resolution methods (StageInfo associated type)
- `BlockEvaluator<'ir>` trait: eval_block, bind_block_args (Ext associated type). Supertrait: ValueStore + StageAccess
- `Interpreter<'ir>`: blanket supertrait of BlockEvaluator (auto-implemented)
- `InterpreterError`: 9 variants (StageResolution nests StageResolutionError)
- `DispatchCache<Entry>`: public, in `dispatch` module

**Step 2: Update CLAUDE.md Interpreter Conventions**

Update the `Interpreter<'ir>` lifetime pattern section to describe the new decomposition.

**Step 3: Commit**

```bash
git add CLAUDE.md
git commit -m "docs: update interpreter conventions for trait decomposition"
```

---

### Task 11: Final validation

**Step 1: Full workspace build**

Run: `cargo build --workspace`

**Step 2: Full test suite**

Run: `cargo nextest run --workspace`
Run: `cargo test --doc --workspace`

**Step 3: Format check**

Run: `cargo fmt --all`

**Step 4: Review snapshot tests**

Run: `cargo insta review` (if any snapshots changed)

**Step 5: Final commit if needed**

```bash
git add -A
git commit -m "chore: final cleanup after interpreter trait decomposition"
```
