# Interpreter Framework Simplification — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Simplify kirin-interpreter's public API: fold EvalBlock into Interpreter, rename EvalCall→CallSemantics, merge InStage/WithStage into Staged, remove type aliases, add prelude and analysis submodule.

**Architecture:** Bottom-up approach — rename/restructure internal types first, then update impls, then update exports, then update all consumers (dialects, derives, tests). Each task is independently compilable and testable.

**Tech Stack:** Rust, cargo nextest, cargo fmt, proc-macro2/quote/syn (derive macros)

---

### Task 1: Rename EvalCall → CallSemantics (trait definition + eval/mod.rs)

**Files:**
- Modify: `crates/kirin-interpreter/src/eval/call.rs:14-24`
- Modify: `crates/kirin-interpreter/src/eval/mod.rs:5`

**Step 1: Rename the trait in call.rs**

In `crates/kirin-interpreter/src/eval/call.rs`, rename the trait and update doc references:

```rust
// Line 14: rename trait
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

Update doc comments referencing `EvalCall` → `CallSemantics` (lines 7, 10, 28, 36, 75).

Update both blanket impls (line 39: `CallSemantics<'ir, ...> for T`, line 78: same).

**Step 2: Update eval/mod.rs re-export**

```rust
pub use call::{CallSemantics, SSACFGRegion};
```

**Step 3: Run `cargo build -p kirin-interpreter` to verify**

Expected: Many errors from internal consumers of `EvalCall` — that's fine, we'll fix them in subsequent tasks.

**Step 4: Commit**

```
refactor(interpreter): rename EvalCall trait to CallSemantics
```

---

### Task 2: Update all internal references to EvalCall → CallSemantics

**Files:**
- Modify: `crates/kirin-interpreter/src/lib.rs:19,32`
- Modify: `crates/kirin-interpreter/src/stack/call.rs:9,47`
- Modify: `crates/kirin-interpreter/src/stack/stage.rs:7,50,139`
- Modify: `crates/kirin-interpreter/src/stack/dispatch.rs:9,54`
- Modify: `crates/kirin-interpreter/src/abstract_interp/stage.rs:5,17,36`
- Modify: `crates/kirin-interpreter/src/abstract_interp/fixpoint.rs:8,54,71,396`
- Modify: `crates/kirin-interpreter/src/abstract_interp/interp.rs:39-40` (doc comment)

**Step 1: Replace all `EvalCall` → `CallSemantics` in the files listed above**

In each file, replace the import and all usages. Key changes:

`lib.rs:19`:
```rust
pub use eval::{CallSemantics, SSACFGRegion};
```

`lib.rs:32`:
```rust
pub use kirin_derive_interpreter::EvalCall as CallSemantics;
```
Note: The derive macro is still named `EvalCall` (proc-macro crate name unchanged). We re-export it under the new name. We'll update the derive output in Task 8.

`stack/call.rs:9`: replace `EvalCall` with `CallSemantics` in import and line 47 bound.

`stack/stage.rs:7`: replace `EvalCall` with `CallSemantics` in import and lines 50, 139.

`stack/dispatch.rs:9`: replace `EvalCall` with `CallSemantics` in import and line 54 bound.

`abstract_interp/stage.rs:5`: replace `EvalCall` with `CallSemantics` in import and lines 17, 36.

`abstract_interp/fixpoint.rs:8`: replace `EvalCall` with `CallSemantics` in import and lines 54, 71, 396.

`abstract_interp/interp.rs:39-40`: update doc comment `EvalCall` → `CallSemantics`.

**Step 2: Run `cargo build -p kirin-interpreter`**

Expected: PASS (all internal refs updated).

**Step 3: Commit**

```
refactor(interpreter): update all internal EvalCall references to CallSemantics
```

---

### Task 3: Fold EvalBlock into Interpreter trait

**Files:**
- Modify: `crates/kirin-interpreter/src/interpreter.rs` (add `eval_block` method)
- Delete contents from: `crates/kirin-interpreter/src/eval/block.rs` (move impls to stack/abstract modules)
- Modify: `crates/kirin-interpreter/src/eval/mod.rs` (remove EvalBlock export)
- Modify: `crates/kirin-interpreter/src/lib.rs` (remove EvalBlock export)
- Modify: `crates/kirin-interpreter/src/stack/interp.rs` (add eval_block to Interpreter impl — but StackInterpreter doesn't impl Interpreter directly, the trait impl is on the struct)
- Modify: `crates/kirin-interpreter/src/abstract_interp/interp.rs` (add eval_block to Interpreter impl)

**Step 1: Add `eval_block` to the `Interpreter` trait**

In `crates/kirin-interpreter/src/interpreter.rs`, after `bind_block_args` (line 153), add:

```rust
    /// Execute a body block whose arguments have already been bound.
    ///
    /// Returns the [`Continuation`] produced by the block's terminator.
    /// The caller must call [`bind_block_args`](Self::bind_block_args) first
    /// to write values into the block's argument SSA slots.
    fn eval_block<L: Dialect>(
        &mut self,
        stage: &'ir StageInfo<L>,
        block: Block,
    ) -> Result<Continuation<Self::Value, Self::Ext>, Self::Error>;
```

Add `Block` and `Continuation` to the imports at the top if not already there:
```rust
use crate::Continuation;
```

Note: `Block` is already imported from `kirin_ir`.

**Step 2: Move the StackInterpreter EvalBlock impl into its Interpreter impl**

The StackInterpreter doesn't have a single `impl Interpreter` block — it uses a separate `Interpreter` impl. We need to find it. Check `stack/interp.rs` for the Interpreter trait impl. Actually, looking at the code, the `Interpreter` impl for `StackInterpreter` is in `stack/interp.rs`. Add `eval_block` there.

From `eval/block.rs` lines 27-57, move the implementation into the `Interpreter` impl for `StackInterpreter`. The bounds on the `eval_block` method will need to be method-level where clauses since the Interpreter impl doesn't have `L` or `HasStageInfo<L>` bounds.

```rust
fn eval_block<L: Dialect>(
    &mut self,
    stage: &'ir StageInfo<L>,
    block: Block,
) -> Result<Continuation<V, crate::ConcreteExt>, E>
where
    S: HasStageInfo<L>,
    S: SupportsStageDispatch<
            crate::stack::FrameDispatchAction<'ir, V, S, E, G>,
            crate::stack::DynFrameDispatch<'ir, V, S, E, G>,
            E,
        >,
    for<'a> S: SupportsStageDispatch<crate::stack::PushCallFrameDynAction<'a, 'ir, V, S, E, G>, (), E>,
    L: Interpretable<'ir, Self, L>,
{
    let saved_cursor = self.current_cursor()?;
    let first = block.first_statement(stage);
    self.set_current_cursor(first)?;
    let v = self.run_nested_calls(|_interp, is_yield| is_yield)?;
    self.set_current_cursor(saved_cursor)?;
    Ok(Continuation::Yield(v))
}
```

**Step 3: Move the AbstractInterpreter EvalBlock impl into its Interpreter impl**

From `eval/block.rs` lines 59-109, move into `abstract_interp/interp.rs` Interpreter impl (starting around line 294).

```rust
fn eval_block<L: Dialect>(
    &mut self,
    stage: &'ir StageInfo<L>,
    block: Block,
) -> Result<Continuation<V, std::convert::Infallible>, E>
where
    S: HasStageInfo<L>,
    L: Interpretable<'ir, Self, L> + 'ir,
{
    for stmt in block.statements(stage) {
        let def: &L = stmt.definition(stage);
        let control = def.interpret(self)?;
        match control {
            Continuation::Continue => {}
            Continuation::Call {
                callee,
                stage: callee_stage,
                args,
                result,
            } => {
                let handler = self
                    .call_handler
                    .expect("call_handler not set: analyze() must be used as entry point");
                let analysis = handler(self, callee, callee_stage, &args)?;
                let return_val = analysis.return_value().cloned().unwrap_or_else(V::bottom);
                self.write(result, return_val)?;
            }
            other => return Ok(other),
        }
    }
    if let Some(term) = block.terminator(stage) {
        let def: &L = term.definition(stage);
        let control = def.interpret(self)?;
        Ok(control)
    } else {
        Err(InterpreterError::MissingEntry.into())
    }
}
```

**Step 4: Remove eval/block.rs contents and EvalBlock exports**

- Replace `crates/kirin-interpreter/src/eval/block.rs` with an empty file or remove it entirely
- Update `crates/kirin-interpreter/src/eval/mod.rs` to remove the `block` module and `EvalBlock` export
- Remove `EvalBlock` from `lib.rs` line 19

**Step 5: Update fixpoint.rs to use `Interpreter::eval_block` instead of `EvalBlock::<'ir, L>::eval_block`**

In `crates/kirin-interpreter/src/abstract_interp/fixpoint.rs`:
- Line 8: remove `EvalBlock` from imports
- Line 161: change `EvalBlock::<'ir, L>::eval_block(self, stage, block)?` → `self.eval_block(stage, block)?`
- Line 177: same change

**Step 6: Run `cargo build -p kirin-interpreter`**

Expected: PASS.

**Step 7: Commit**

```
refactor(interpreter): fold EvalBlock trait into Interpreter
```

---

### Task 4: Merge InStage + WithStage into Staged

**Files:**
- Modify: `crates/kirin-interpreter/src/stage.rs` (replace InStage/WithStage with Staged)
- Modify: `crates/kirin-interpreter/src/interpreter.rs` (update builder methods)
- Modify: `crates/kirin-interpreter/src/stack/stage.rs` (update all impl blocks)
- Modify: `crates/kirin-interpreter/src/abstract_interp/stage.rs` (update all impl blocks)
- Modify: `crates/kirin-interpreter/src/stack/dispatch.rs` (update references)
- Modify: `crates/kirin-interpreter/src/lib.rs` (update export)

**Step 1: Replace stage.rs with unified Staged type**

```rust
use std::marker::PhantomData;

use kirin_ir::{CompileStage, Dialect, HasStageInfo, StageInfo};

use crate::{Interpreter, InterpreterError};

/// Extract the stage ID from a `StageInfo`, panicking if it is not attached
/// to a pipeline stage.
pub(crate) fn expect_stage_id<L: Dialect>(stage: &StageInfo<L>) -> CompileStage {
    stage
        .stage_id()
        .expect("stage info must be attached to a pipeline stage")
}

/// Typed-stage API builder for stage-scoped interpreter operations.
///
/// Constructed via [`Interpreter::in_stage`] (resolves active stage eagerly)
/// or [`Interpreter::with_stage`] (takes explicit stage reference).
pub struct Staged<'a, 'ir, I, L: Dialect> {
    pub(crate) interp: &'a mut I,
    pub(crate) stage: &'ir StageInfo<L>,
}
```

**Step 2: Update interpreter.rs builder methods**

Replace `in_stage` and `with_stage` to both return `Staged`:

```rust
use crate::stage::Staged;
// remove: use crate::stage::{InStage, WithStage};

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

fn with_stage<L: Dialect>(&mut self, stage: &'ir StageInfo<L>) -> Staged<'_, 'ir, Self, L> {
    Staged {
        interp: self,
        stage,
    }
}
```

**Step 3: Merge stack/stage.rs impl blocks**

Replace the two impl blocks (`InStage<StackInterpreter>` and `WithStage<StackInterpreter>`) with a single `Staged<StackInterpreter>` impl. All methods now use `self.stage` directly instead of resolving:

```rust
use kirin_ir::{
    Dialect, HasStageInfo, SpecializedFunction, StageInfo, StageMeta, SupportsStageDispatch,
};

use super::{DynFrameDispatch, FrameDispatchAction, PushCallFrameDynAction, StackInterpreter};
use crate::{
    CallSemantics, Continuation, Interpretable, Interpreter,
    InterpreterError, stage::Staged,
};

impl<'a, 'ir, V, S, E, G, L> Staged<'a, 'ir, StackInterpreter<'ir, V, S, E, G>, L>
where
    V: Clone + 'ir,
    E: From<InterpreterError> + 'ir,
    S: StageMeta + 'ir,
    G: 'ir,
    L: Dialect + Interpretable<'ir, StackInterpreter<'ir, V, S, E, G>, L> + 'ir,
{
    /// Execute the current statement's dialect semantics.
    pub fn step(self) -> Result<Continuation<V, crate::ConcreteExt>, E> {
        self.interp.step_with_stage::<L>(self.stage)
    }

    /// Apply cursor mutations for a continuation with this stage.
    pub fn advance(self, control: &Continuation<V, crate::ConcreteExt>) -> Result<(), E>
    where
        S: SupportsStageDispatch<
                FrameDispatchAction<'ir, V, S, E, G>,
                DynFrameDispatch<'ir, V, S, E, G>,
                E,
            >,
        for<'b> S: SupportsStageDispatch<PushCallFrameDynAction<'b, 'ir, V, S, E, G>, (), E>,
    {
        self.interp
            .advance_frame_with_stage::<L>(self.stage, control)?;
        if let Continuation::Call {
            callee,
            stage: callee_stage,
            args,
            ..
        } = control
        {
            self.interp
                .push_call_frame_with_args(*callee, *callee_stage, args)?;
        }
        Ok(())
    }

    /// Call a specialized function and return its result value.
    pub fn call(self, callee: SpecializedFunction, args: &[V]) -> Result<V, E>
    where
        L: CallSemantics<'ir, StackInterpreter<'ir, V, S, E, G>, L, Result = V>,
    {
        self.interp.call_with_stage::<L>(callee, self.stage, args)
    }

    /// Run statements until Return, Halt, or Call.
    pub fn run(self) -> Result<Continuation<V, crate::ConcreteExt>, E>
    where
        S: HasStageInfo<L>,
        S: SupportsStageDispatch<
                FrameDispatchAction<'ir, V, S, E, G>,
                DynFrameDispatch<'ir, V, S, E, G>,
                E,
            >,
        for<'b> S: SupportsStageDispatch<PushCallFrameDynAction<'b, 'ir, V, S, E, G>, (), E>,
    {
        self.interp.drive_loop(
            false,
            true,
            |interp: &mut StackInterpreter<'ir, V, S, E, G>| interp.in_stage::<L>().step(),
            |interp: &mut StackInterpreter<'ir, V, S, E, G>, control| {
                interp.in_stage::<L>().advance(control)
            },
        )
    }

    /// Run statements until a breakpoint, Return, Halt, or Call.
    pub fn run_until_break(self) -> Result<Continuation<V, crate::ConcreteExt>, E>
    where
        S: HasStageInfo<L>,
        S: SupportsStageDispatch<
                FrameDispatchAction<'ir, V, S, E, G>,
                DynFrameDispatch<'ir, V, S, E, G>,
                E,
            >,
        for<'b> S: SupportsStageDispatch<PushCallFrameDynAction<'b, 'ir, V, S, E, G>, (), E>,
    {
        self.interp.drive_loop(
            true,
            false,
            |interp: &mut StackInterpreter<'ir, V, S, E, G>| interp.in_stage::<L>().step(),
            |interp: &mut StackInterpreter<'ir, V, S, E, G>, control| {
                interp.in_stage::<L>().advance(control)
            },
        )
    }

    pub(crate) fn push_call_frame(self, callee: SpecializedFunction, args: &[V]) -> Result<(), E>
    where
        S: SupportsStageDispatch<
                FrameDispatchAction<'ir, V, S, E, G>,
                DynFrameDispatch<'ir, V, S, E, G>,
                E,
            >,
    {
        self.interp
            .push_call_frame_with_stage::<L>(callee, self.stage, args)
    }
}
```

**Step 4: Merge abstract_interp/stage.rs impl blocks**

Replace both impl blocks with single `Staged<AbstractInterpreter>`:

```rust
use kirin_ir::{Dialect, HasStageInfo, SpecializedFunction, StageMeta, SupportsStageDispatch};

use super::{AbstractInterpreter, fixpoint::AnalyzeDynAction};
use crate::{
    AbstractValue, CallSemantics, Interpretable, Interpreter, InterpreterError,
    result::AnalysisResult, stage::{Staged, expect_stage_id},
};

impl<'a, 'ir, V, S, E, G, L> Staged<'a, 'ir, AbstractInterpreter<'ir, V, S, E, G>, L>
where
    V: AbstractValue + Clone + 'ir,
    E: From<InterpreterError> + 'ir,
    S: StageMeta + HasStageInfo<L> + 'ir,
    G: 'ir,
    L: Dialect
        + Interpretable<'ir, AbstractInterpreter<'ir, V, S, E, G>, L>
        + CallSemantics<'ir, AbstractInterpreter<'ir, V, S, E, G>, L, Result = AnalysisResult<V>>
        + 'ir,
    for<'x> S: SupportsStageDispatch<AnalyzeDynAction<'x, 'ir, V, S, E, G>, AnalysisResult<V>, E>,
{
    /// Analyze a specialized function in this stage.
    pub fn analyze(self, callee: SpecializedFunction, args: &[V]) -> Result<AnalysisResult<V>, E> {
        let stage_id = expect_stage_id(self.stage);
        self.interp.call_handler = Some(AbstractInterpreter::analyze);
        self.interp
            .analyze_with_stage_id::<L>(callee, stage_id, args)
    }
}
```

**Step 5: Update stack/dispatch.rs references**

Replace `WithStage` → `Staged` in `PushCallFrameDynAction::run` (line 162-164) and `CallDynAction::run` (line 66).

**Step 6: Update lib.rs export**

```rust
pub use stage::Staged;
// remove: pub use stage::{InStage, WithStage};
```

**Step 7: Run `cargo build -p kirin-interpreter`**

Expected: PASS.

**Step 8: Commit**

```
refactor(interpreter): merge InStage/WithStage into unified Staged type
```

---

### Task 5: Remove ConcreteContinuation and AbstractContinuation type aliases

**Files:**
- Modify: `crates/kirin-interpreter/src/control.rs:62-65` (remove aliases)
- Modify: `crates/kirin-interpreter/src/lib.rs:17` (remove from exports)
- Modify: `crates/kirin-interpreter/src/stack/stage.rs` (replace usages)
- Modify: `crates/kirin-interpreter/src/stack/exec.rs` (replace usages)
- Modify: `crates/kirin-interpreter/src/stack/dispatch.rs` (replace usages)
- Modify: `crates/kirin-interpreter/src/stack/transition.rs` (replace usages)
- Modify: `crates/kirin-interpreter/src/abstract_interp/fixpoint.rs:244` (replace usage)

**Step 1: Remove type aliases from control.rs**

Delete lines 61-65 in `control.rs`:
```rust
// DELETE:
// pub type ConcreteContinuation<V> = Continuation<V, ConcreteExt>;
// pub type AbstractContinuation<V> = Continuation<V>;
```

**Step 2: Replace all usages**

In every file that imports `ConcreteContinuation`, replace with `Continuation<V, ConcreteExt>`. In every file that imports `AbstractContinuation`, replace with `Continuation<V>`.

Key files:
- `stack/exec.rs`: Replace `ConcreteContinuation<V>` with `Continuation<V, ConcreteExt>` throughout (lines 4, 12, 14, 16, 22, 46, 64, 87, 89, 90)
- `stack/dispatch.rs`: Same (lines 9, 12, 14, 72, 85)
- `stack/stage.rs`: Same (lines 7, 26, 33, 58, 78, 107, 112)
- `stack/transition.rs`: Same (lines 5, 67, 82)
- `abstract_interp/fixpoint.rs`: Replace `AbstractContinuation<V>` with `Continuation<V>` (lines 8, 244)

**Step 3: Update lib.rs export**

```rust
pub use control::{Args, ConcreteExt, Continuation};
// removed: AbstractContinuation, ConcreteContinuation
```

**Step 4: Run `cargo build -p kirin-interpreter`**

Expected: PASS.

**Step 5: Commit**

```
refactor(interpreter): remove ConcreteContinuation/AbstractContinuation type aliases
```

---

### Task 6: Move dispatch statics off the Interpreter trait

**Files:**
- Modify: `crates/kirin-interpreter/src/interpreter.rs` (remove `map_dispatch_miss`, `dispatch_in_pipeline`)
- Create: `crates/kirin-interpreter/src/dispatch.rs` (free functions)
- Modify: `crates/kirin-interpreter/src/lib.rs` (add module)
- Modify: `crates/kirin-interpreter/src/stack/call.rs` (update calls)
- Modify: `crates/kirin-interpreter/src/abstract_interp/fixpoint.rs` (update calls)

**Step 1: Create dispatch.rs with free functions**

```rust
use kirin_ir::{CompileStage, Pipeline, StageDispatchMiss, StageMeta, SupportsStageDispatch};

use crate::InterpreterError;

/// Convert a stage-dispatch miss into the framework error model.
pub fn map_dispatch_miss<E: From<InterpreterError>>(
    stage_id: CompileStage,
    miss: StageDispatchMiss,
) -> E {
    match miss {
        StageDispatchMiss::MissingStage => InterpreterError::MissingStage { stage: stage_id },
        StageDispatchMiss::MissingDialect => {
            InterpreterError::MissingStageDialect { stage: stage_id }
        }
    }
    .into()
}

/// Dispatch a runtime action against `stage_id` using `pipeline`, mapping
/// dispatch misses to [`InterpreterError`] variants.
pub fn dispatch_in_pipeline<S, A, R, E>(
    pipeline: &Pipeline<S>,
    stage_id: CompileStage,
    action: &mut A,
) -> Result<R, E>
where
    S: StageMeta + SupportsStageDispatch<A, R, E>,
    E: From<InterpreterError>,
{
    pipeline.dispatch_stage_or_else(stage_id, action, |miss| {
        map_dispatch_miss(stage_id, miss)
    })
}
```

**Step 2: Remove from Interpreter trait**

Remove the `map_dispatch_miss` and `dispatch_in_pipeline` methods from `interpreter.rs` (lines 92-120). Also remove `StageDispatchMiss` and `SupportsStageDispatch` from the imports.

**Step 3: Update callers**

In `stack/call.rs` line 37: `Self::dispatch_in_pipeline(...)` → `crate::dispatch::dispatch_in_pipeline(...)`

In `stack/call.rs` line 128: same.

In `abstract_interp/fixpoint.rs` line 41: `Self::dispatch_in_pipeline(...)` → `crate::dispatch::dispatch_in_pipeline(...)`

**Step 4: Add module to lib.rs**

```rust
mod dispatch;
```

Keep `dispatch` as `pub(crate)` — these are internal utilities, not for public consumption.

**Step 5: Run `cargo build -p kirin-interpreter`**

Expected: PASS.

**Step 6: Commit**

```
refactor(interpreter): move dispatch statics off Interpreter trait into free functions
```

---

### Task 7: Move summary _in_stage methods to Staged

**Files:**
- Modify: `crates/kirin-interpreter/src/abstract_interp/interp.rs` (remove `_in_stage` methods)
- Modify: `crates/kirin-interpreter/src/abstract_interp/stage.rs` (add summary methods to Staged)

**Step 1: Add summary methods to Staged<AbstractInterpreter>**

In `abstract_interp/stage.rs`, add a second impl block for `Staged<AbstractInterpreter>` with relaxed bounds (no `CallSemantics` or dispatch bounds needed for summary access):

```rust
impl<'a, 'ir, V, S, E, G, L> Staged<'a, 'ir, AbstractInterpreter<'ir, V, S, E, G>, L>
where
    V: AbstractValue + Clone + 'ir,
    S: StageMeta + 'ir,
    L: Dialect,
{
    /// Look up the best cached summary for `callee` in this stage.
    pub fn summary(
        &self,
        callee: SpecializedFunction,
        args: &[V],
    ) -> Option<&AnalysisResult<V>> {
        let stage_id = expect_stage_id(self.stage);
        self.interp.summary_in_stage(stage_id, callee, args)
    }

    /// Look up the full summary cache for `callee` in this stage.
    pub fn summary_cache(
        &self,
        callee: SpecializedFunction,
    ) -> Option<&SummaryCache<V>> {
        let stage_id = expect_stage_id(self.stage);
        self.interp.summary_cache_in_stage(stage_id, callee)
    }

    /// Return a builder for inserting a function summary in this stage.
    pub fn insert_summary(
        self,
        callee: SpecializedFunction,
    ) -> SummaryInserter<'a, 'ir, V, S, E, G> {
        let stage_id = expect_stage_id(self.stage);
        self.interp.insert_summary_in_stage(stage_id, callee)
    }

    /// Mark all computed entries for `callee` in this stage as invalidated.
    pub fn invalidate_summary(
        &mut self,
        callee: SpecializedFunction,
    ) -> usize {
        let stage_id = expect_stage_id(self.stage);
        self.interp.invalidate_summary_in_stage(stage_id, callee)
    }

    /// Unconditionally remove all summaries for `callee` in this stage.
    pub fn remove_summary(
        &mut self,
        callee: SpecializedFunction,
    ) -> bool {
        let stage_id = expect_stage_id(self.stage);
        self.interp.remove_summary_in_stage(stage_id, callee)
    }
}
```

Note: The `_in_stage` methods on `AbstractInterpreter` stay for now as `pub(crate)` since the `Staged` methods delegate to them. Only the `pub` duplicates (the non-`_in_stage` convenience methods that resolve active stage) are removed.

**Step 2: Remove public non-`_in_stage` convenience methods from AbstractInterpreter**

In `abstract_interp/interp.rs`, change these methods from `pub` to `pub(crate)` or remove entirely:
- `summary()` (line 191) → remove (use `interp.in_stage::<L>().summary()` instead)
- `summary_cache()` (line 215) → remove
- `insert_summary()` (line 229) → remove
- `invalidate_summary()` (line 249) → remove
- `remove_summary()` (line 275) → remove

Keep the `_in_stage` variants as `pub(crate)` since they're used by the `Staged` methods.

Rename the `_in_stage` methods to drop the suffix since they're no longer public API (optional, can keep for clarity).

**Step 3: Run `cargo build -p kirin-interpreter && cargo nextest run -p kirin-interpreter`**

Expected: PASS (no external callers of these methods yet).

**Step 4: Commit**

```
refactor(interpreter): move summary methods to Staged, remove public _in_stage duplicates
```

---

### Task 8: Update derive macro output (EvalCall → CallSemantics)

**Files:**
- Modify: `crates/kirin-derive-interpreter/src/eval_call/emit.rs` (change generated trait path)
- Modify: `crates/kirin-derive-interpreter/src/eval_call/tests.rs` (update snapshot if applicable)

**Step 1: Update generated trait references**

In `eval_call/emit.rs`, replace all occurrences of `#interp_crate::EvalCall` with `#interp_crate::CallSemantics` in the quote! blocks:

- Line 26: `impl ... #interp_crate::CallSemantics<'__ir, ...>`
- Line 31: `#wrapper_ty: #interp_crate::CallSemantics<...>`
- Line 34: `<#wrapper_ty as #interp_crate::CallSemantics<...>>::Result`
- Line 51: `impl ... #interp_crate::CallSemantics<'__ir, ...>`
- Line 127: `<#first_wrapper as #interp_crate::CallSemantics<...>>::Result`
- Line 138: `#ty: #interp_crate::CallSemantics<...>`
- Line 142: `#ty: #interp_crate::CallSemantics<...>`
- Line 150: `impl ... #interp_crate::CallSemantics<'__ir, ...>`

**Step 2: Keep the derive macro name as `EvalCall` for now**

The `#[proc_macro_derive(EvalCall, ...)]` in `lib.rs` stays as `EvalCall` — this is the derive attribute name. We can optionally rename it to `CallSemantics` as a follow-up, but it's cosmetic and the re-export in `kirin-interpreter/src/lib.rs` already handles it:

```rust
#[cfg(feature = "derive")]
pub use kirin_derive_interpreter::EvalCall as CallSemantics;
```

Actually — we should rename the derive macro attribute too, since users write `#[derive(EvalCall)]`. Let's rename it:

In `crates/kirin-derive-interpreter/src/lib.rs`:
```rust
#[proc_macro_derive(CallSemantics, attributes(wraps, callable, kirin))]
pub fn derive_call_semantics(input: TokenStream) -> TokenStream {
    // same body
}
```

Update `kirin-interpreter/src/lib.rs`:
```rust
#[cfg(feature = "derive")]
pub use kirin_derive_interpreter::CallSemantics;
```

**Step 3: Run tests**

```bash
cargo nextest run -p kirin-derive-interpreter
cargo build -p kirin-interpreter
```

Expected: PASS.

**Step 4: Commit**

```
refactor(derive-interpreter): rename EvalCall derive macro to CallSemantics
```

---

### Task 9: Add prelude and analysis submodule

**Files:**
- Modify: `crates/kirin-interpreter/src/lib.rs` (add prelude module and analysis module)

**Step 1: Add prelude module**

In `lib.rs`, add:

```rust
/// Essentials for dialect authors implementing operational semantics.
///
/// ```rust
/// use kirin_interpreter::prelude::*;
/// ```
pub mod prelude {
    pub use crate::{
        Interpreter,
        Interpretable,
        Continuation,
        InterpreterError,
        BranchCondition,
        CallSemantics,
        SSACFGRegion,
        Args,
    };
}
```

**Step 2: Add analysis submodule**

```rust
/// Types for abstract interpretation and fixpoint analysis.
pub mod analysis {
    pub use crate::{
        AbstractValue,
        WideningStrategy,
        FixpointState,
        SummaryCache,
        SummaryEntry,
        AnalysisResult,
        DedupScheduler,
    };
}
```

**Step 3: Run `cargo build -p kirin-interpreter`**

Expected: PASS.

**Step 4: Commit**

```
feat(interpreter): add prelude and analysis submodules for tiered API
```

---

### Task 10: Update dialect crates

**Files:**
- Modify: `crates/kirin-scf/src/interpret_impl.rs` (remove EvalBlock import/bound)
- Modify: `crates/kirin-cf/src/interpret_impl.rs` (no changes needed — doesn't use EvalBlock)
- Modify: `crates/kirin-function/src/interpret_impl.rs` (no changes needed)
- Modify: `crates/kirin-arith/src/interpret_impl.rs` (no changes needed)
- Modify: `crates/kirin-constant/src/interpret_impl.rs` (no changes needed)

**Step 1: Update kirin-scf**

In `crates/kirin-scf/src/interpret_impl.rs`:
- Line 3: Remove `EvalBlock` from import
- Line 51: Change `I: Interpreter<'ir> + EvalBlock<'ir, L>` → `I: Interpreter<'ir>`
- Line 91: Same change for `StructuredControlFlow`

```rust
use kirin_interpreter::{
    BranchCondition, Continuation, Interpretable, Interpreter, InterpreterError,
};
```

```rust
impl<'ir, I, L, T> Interpretable<'ir, I, L> for For<T>
where
    I: Interpreter<'ir>,  // was: I: Interpreter<'ir> + EvalBlock<'ir, L>,
    // ... rest unchanged
```

**Step 2: Run `cargo build --workspace`**

Expected: PASS.

**Step 3: Commit**

```
refactor(scf): remove EvalBlock bounds after fold into Interpreter
```

---

### Task 11: Update test files and kirin-test-languages

**Files:**
- Modify: `crates/kirin-test-languages/src/composite_language.rs` (EvalCall derive → CallSemantics)
- Modify: `crates/kirin-interpreter/tests/derive_macros.rs` (update derive and trait imports)
- Modify: `crates/kirin-interpreter/tests/stage_dispatch.rs` (update derive)
- Modify: `crates/kirin-interpreter/tests/stack_interp.rs` (verify no changes needed)
- Modify: `crates/kirin-interpreter/tests/abstract_fixpoint.rs` (verify no changes needed)

**Step 1: Update kirin-test-languages**

In `crates/kirin-test-languages/src/composite_language.rs`:
```rust
// Line 4:
use kirin_derive_interpreter::{CallSemantics, Interpretable};
// Line 8:
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, Interpretable, CallSemantics)]
```

**Step 2: Update derive_macros.rs test**

```rust
// Line 4:
use kirin_derive_interpreter::{CallSemantics, Interpretable};
// Line 7:
use kirin_interpreter::{
    BranchCondition, CallSemantics as CallSemanticsTrait, Interpreter, InterpreterError, StackInterpreter,
};
// Line 15:
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, Interpretable, CallSemantics)]
```

Update all `EvalCallTrait` references to `CallSemanticsTrait`.

**Step 3: Update stage_dispatch.rs test**

```rust
// Line 4:
use kirin_derive_interpreter::{CallSemantics, Interpretable};
// Lines 132, 145, 506:
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, Interpretable, CallSemantics)]
```

**Step 4: Run full test suite**

```bash
cargo nextest run --workspace
cargo test --doc --workspace
```

Expected: PASS.

**Step 5: Commit**

```
refactor(tests): update all tests for CallSemantics rename and EvalBlock removal
```

---

### Task 12: Format and final verification

**Step 1: Format everything**

```bash
cargo fmt --all
```

**Step 2: Run full test suite**

```bash
cargo nextest run --workspace
cargo test --doc --workspace
```

**Step 3: Review snapshot tests**

```bash
cargo insta review
```

Accept any snapshot changes if they only reflect the rename.

**Step 4: Commit any formatting changes**

```
chore: cargo fmt
```

---

## Dependency Graph

```
Task 1 (rename trait) → Task 2 (internal refs) → Task 3 (fold EvalBlock)
                                                       ↓
Task 4 (merge Staged) ←──────────────────────────── depends on Task 3
       ↓
Task 5 (remove aliases) ← independent of 4, needs 1-3
Task 6 (dispatch statics) ← independent of 4-5, needs 1-3
Task 7 (summary to Staged) ← needs Task 4
Task 8 (derive macro) ← needs Task 1
Task 9 (prelude/analysis) ← needs Tasks 1-6
Task 10 (dialects) ← needs Tasks 1-4
Task 11 (tests) ← needs Tasks 1-8
Task 12 (format/verify) ← needs all
```

Parallelizable groups:
- **Group A** (sequential): Tasks 1 → 2 → 3
- **Group B** (after Group A, parallelizable): Tasks 4, 5, 6
- **Group C** (after Group B): Tasks 7, 8, 9, 10
- **Group D** (after all): Tasks 11, 12
