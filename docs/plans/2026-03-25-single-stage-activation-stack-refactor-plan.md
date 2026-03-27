# SingleStage Activation Stack Refactor Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Refactor `kirin-interpreter-2::interpreter::SingleStage` to own the full same-stage activation stack, then migrate `kirin-function::interpreter2` to use shell-owned invocation instead of a function-local call-frame machine.

**Architecture:** Move loose shell state (`stage`, SSA bindings, cursor stack, after-statement state) into `Frame<V, Activation>` stored in a private `FrameStack` inside `SingleStage`. Add a generic split between call resolution (`ResolveCall`, implemented by call-like ops or request types) and specialized-function execution (`Invoke`, implemented by `SingleStage`). Simplify `kirin-function::interpreter2` to use shared `effect::Flow`/`effect::Stateless` and shell-owned invocation/return.

**Tech Stack:** Rust 2024, `kirin-interpreter-2`, `kirin-function`, `kirin-cf`, `rustc_hash::FxHashMap`, existing `Frame`/`FrameStack`, `cargo test`, `cargo clippy`

---

## File Structure

**Create:**
- `crates/kirin-interpreter-2/src/interpreter/resolve_call.rs`
- `crates/kirin-interpreter-2/src/interpreter/invoke.rs`
- `crates/kirin-interpreter-2/src/interpreter/single_stage/mod.rs`
- `crates/kirin-interpreter-2/src/interpreter/single_stage/activation.rs`
- `crates/kirin-interpreter-2/src/interpreter/single_stage/frame_ops.rs`
- `crates/kirin-interpreter-2/src/interpreter/single_stage/traits.rs`
- `crates/kirin-interpreter-2/src/tests/invoke.rs`

**Modify:**
- `crates/kirin-interpreter-2/src/interpreter/mod.rs`
- `crates/kirin-interpreter-2/src/interpreter/block_bindings.rs`
- `crates/kirin-interpreter-2/src/interpreter/typed_stage.rs`
- `crates/kirin-interpreter-2/src/effect.rs`
- `crates/kirin-interpreter-2/src/lib.rs`
- `crates/kirin-interpreter-2/src/tests/composite_exec.rs`
- `crates/kirin-function/src/interpreter2/mod.rs`
- `crates/kirin-function/src/interpreter2/lifted.rs`
- `crates/kirin-function/src/interpreter2/tests/harness.rs`
- `crates/kirin-function/src/interpreter2/tests/mod.rs`
- `crates/kirin-function/src/interpreter2/tests/programs.rs`
- `docs/design/2026-03-24-kirin-interpreter-machine/index.md`
- `docs/design/2026-03-24-kirin-interpreter-machine/interpreter-shells.md`
- `docs/design/2026-03-24-kirin-interpreter-machine/machine.md`

**Delete:**
- `crates/kirin-interpreter-2/src/interpreter/value_bindings.rs`
- `crates/kirin-interpreter-2/src/interpreter/shell_state.rs`
- `crates/kirin-interpreter-2/src/interpreter/resume_seed.rs`
- `crates/kirin-interpreter-2/src/interpreter/single_stage.rs`
- `crates/kirin-function/src/interpreter2/machine.rs`
- `crates/kirin-function/src/interpreter2/runtime.rs`

**Tests:**
- `crates/kirin-interpreter-2/src/tests/invoke.rs`
- `crates/kirin-interpreter-2/src/tests/composite_exec.rs`
- `crates/kirin-function/src/interpreter2/tests/mod.rs`

---

## Chunk 1: Rebuild `SingleStage` Around Activation Frames

### Task 1: Lock the new runtime shape with failing shell tests

**Files:**
- Create: `crates/kirin-interpreter-2/src/tests/invoke.rs`
- Modify: `crates/kirin-interpreter-2/src/tests/mod.rs`
- Test: `crates/kirin-interpreter-2/src/tests/invoke.rs`

- [ ] **Step 1: Write the failing activation-stack tests**

Add tests that prove:

```rust
#[test]
fn invoke_pushes_new_activation_and_preserves_caller_bindings() { /* ... */ }

#[test]
fn return_current_restores_caller_and_writes_product_results() { /* ... */ }

#[test]
fn flow_stay_leaves_current_cursor_unchanged() { /* ... */ }
```

Use existing inline test languages/builders from `crates/kirin-interpreter-2/src/tests/composite_exec.rs` where possible. Build the tests so they call future `Invoke::invoke` / `Invoke::return_current` rather than the soon-to-be-deleted binding-swap helpers.

- [ ] **Step 2: Run the new test file and verify it fails**

Run: `cargo test -p kirin-interpreter-2 invoke -- --nocapture`

Expected: FAIL with missing `Invoke` trait / missing `Flow::Stay` / missing activation-stack behavior.

- [ ] **Step 3: Commit the failing test scaffold**

```bash
git add crates/kirin-interpreter-2/src/tests/mod.rs crates/kirin-interpreter-2/src/tests/invoke.rs
git commit -m "test(interpreter): add activation stack shell tests"
```

### Task 2: Introduce call-resolution and invocation traits

**Files:**
- Create: `crates/kirin-interpreter-2/src/interpreter/resolve_call.rs`
- Create: `crates/kirin-interpreter-2/src/interpreter/invoke.rs`
- Modify: `crates/kirin-interpreter-2/src/interpreter/mod.rs`
- Modify: `crates/kirin-interpreter-2/src/lib.rs`
- Test: `crates/kirin-interpreter-2/src/tests/invoke.rs`

- [ ] **Step 1: Add the new public interpreter traits**

Create the traits with these signatures:

```rust
pub trait ResolveCall<'ir, I: Interpreter<'ir>> {
    fn resolve_call(
        &self,
        interp: &I,
        args: &[I::Value],
    ) -> Result<SpecializedFunction, I::Error>;
}
```

```rust
pub trait Invoke<'ir>: Interpreter<'ir> {
    fn invoke(
        &mut self,
        callee: SpecializedFunction,
        args: &[Self::Value],
        results: &[ResultValue],
    ) -> Result<(), Self::Error>;

    fn return_current(
        &mut self,
        value: Self::Value,
    ) -> Result<effect::Flow<Self::Value>, Self::Error>;
}
```

Export both from `interpreter::mod`, and expose `interpreter` in the prelude the same way the current module-oriented API works.

- [ ] **Step 2: Run a narrow compile check**

Run: `cargo check -p kirin-interpreter-2`

Expected: FAIL in `SingleStage` and tests because the new traits are declared but not implemented.

- [ ] **Step 3: Commit the trait surface**

```bash
git add crates/kirin-interpreter-2/src/interpreter/resolve_call.rs crates/kirin-interpreter-2/src/interpreter/invoke.rs crates/kirin-interpreter-2/src/interpreter/mod.rs crates/kirin-interpreter-2/src/lib.rs
git commit -m "feat(interpreter): add call resolution and invoke traits"
```

### Task 3: Move `SingleStage` state into `FrameStack<V, Activation>`

**Files:**
- Create: `crates/kirin-interpreter-2/src/interpreter/single_stage/mod.rs`
- Create: `crates/kirin-interpreter-2/src/interpreter/single_stage/activation.rs`
- Create: `crates/kirin-interpreter-2/src/interpreter/single_stage/frame_ops.rs`
- Create: `crates/kirin-interpreter-2/src/interpreter/single_stage/traits.rs`
- Delete: `crates/kirin-interpreter-2/src/interpreter/single_stage.rs`
- Modify: `crates/kirin-interpreter-2/src/interpreter/mod.rs`
- Test: `crates/kirin-interpreter-2/src/tests/invoke.rs`

- [ ] **Step 1: Verify assumption — existing `Frame` and `FrameStack` carry the right core data**

Read:
- `crates/kirin-interpreter-2/src/frame.rs`
- `crates/kirin-interpreter-2/src/frame_stack.rs`

Confirm the plan still holds:
- `Frame` already stores `SpecializedFunction`, `CompileStage`, and SSA bindings
- `FrameStack` already provides `current`, `current_mut`, `push`, `pop`, `read`, `write`, `write_ssa`

If either assumption is false, STOP and update the plan before continuing.

- [ ] **Step 2: Define `Activation` and `Continuation`**

Put the shell-local state in `activation.rs`:

```rust
pub(crate) struct Activation {
    pub(crate) cursor_stack: Vec<ExecutionCursor>,
    pub(crate) after_statement: Option<Statement>,
    pub(crate) continuation: Option<Continuation>,
}

pub(crate) struct Continuation {
    pub(crate) resume: ExecutionSeed,
    pub(crate) results: Vec<ResultValue>,
}
```

- [ ] **Step 3: Rewrite `SingleStage` storage**

Change `SingleStage` so it stores:

```rust
pipeline: &'ir Pipeline<StageInfo<L>>,
root_stage: CompileStage,
machine: M,
frames: FrameStack<V, Activation>,
breakpoints: FxHashSet<Breakpoint>,
fuel: Option<u64>,
interrupt_requested: bool,
last_stop: Option<M::Stop>,
```

Remove:
- `stage`
- `values`
- `cursor_stack`
- `after_statement`

- [ ] **Step 4: Implement frame-local helpers in `frame_ops.rs`**

Add private helpers for:
- pushing the root specialization frame
- reading the current activation
- mutating the current activation
- computing the entry block for a `SpecializedFunction`
- binding block args into the current frame
- replacing the current cursor/seed
- computing the next resume seed from the current activation

Keep the frame stack private to `SingleStage`; do not expose raw `FrameStack` in public traits.

- [ ] **Step 5: Port trait impls to the top-frame model**

In `traits.rs`, re-implement:
- `ValueStore`
- `Interpreter<'ir>`
- `TypedStage<'ir>`
- `Position<'ir>`
- `Driver<'ir>`
- `BlockBindings<'ir>`

All of them should project over `frames.current()` / `frames.current_mut()`. `TypedStage` should use the top frame stage, falling back to `root_stage` only before the first activation exists.

- [ ] **Step 6: Run the interpreter crate tests**

Run: `cargo test -p kirin-interpreter-2`

Expected: PASS for the existing suite plus the new activation-stack tests.

- [ ] **Step 7: Commit the activation-stack rewrite**

```bash
git add crates/kirin-interpreter-2/src/interpreter crates/kirin-interpreter-2/src/tests
git commit -m "refactor(interpreter): move single-stage state into activation frames"
```

### Task 4: Remove obsolete binding-swap traits and add `Flow::Stay`

**Files:**
- Modify: `crates/kirin-interpreter-2/src/effect.rs`
- Modify: `crates/kirin-interpreter-2/src/interpreter/mod.rs`
- Delete: `crates/kirin-interpreter-2/src/interpreter/value_bindings.rs`
- Delete: `crates/kirin-interpreter-2/src/interpreter/shell_state.rs`
- Delete: `crates/kirin-interpreter-2/src/interpreter/resume_seed.rs`
- Modify: `crates/kirin-interpreter-2/src/tests/invoke.rs`
- Test: `crates/kirin-interpreter-2/src/tests/invoke.rs`

- [ ] **Step 1: Add `Stay` to the shared flow effect**

Update `effect::Flow` to:

```rust
pub enum Flow<Stop> {
    Advance,
    Stay,
    Jump(ExecutionSeed),
    Stop(Stop),
}
```

and map it to `control::Shell::Stay` in `into_shell`.

- [ ] **Step 2: Delete the obsolete traits**

Remove `ValueBindings`, `ShellState`, and `ResumeSeed`, then repair imports and re-exports. Keep the number of public names small; `ResolveCall` and `Invoke` are the only new public call-related traits.

- [ ] **Step 3: Re-run crate checks**

Run: `cargo check -p kirin-interpreter-2 -p kirin-cf -p kirin-function`

Expected: FAIL in `kirin-function::interpreter2` because it still depends on the deleted binding-swap/runtime layer.

- [ ] **Step 4: Commit the trait cleanup**

```bash
git add crates/kirin-interpreter-2/src/effect.rs crates/kirin-interpreter-2/src/interpreter
git commit -m "refactor(interpreter): remove binding swap traits"
```

---

## Chunk 2: Migrate `kirin-function::interpreter2` To Shell-Owned Invocation

### Task 5: Make function `interpreter2` stateless and request-driven

**Files:**
- Modify: `crates/kirin-function/src/interpreter2/mod.rs`
- Modify: `crates/kirin-function/src/interpreter2/lifted.rs`
- Delete: `crates/kirin-function/src/interpreter2/machine.rs`
- Delete: `crates/kirin-function/src/interpreter2/runtime.rs`
- Modify: `crates/kirin-function/src/lib.rs`
- Test: `crates/kirin-function/src/interpreter2/tests/mod.rs`

- [ ] **Step 1: Verify assumption — function `interpreter2` no longer needs a local machine**

Confirm the only behavior left after the shell rewrite is:
- resolve a call target into `SpecializedFunction`
- invoke the shell
- build a product and return through the shell

If any persistent function-local state remains necessary, STOP and document why before deleting `machine.rs`.

- [ ] **Step 2: Switch the module to shared effect types**

In `interpreter2/mod.rs`, replace the local aliases so the module uses:

```rust
pub type Effect<V> = kirin_interpreter_2::effect::Flow<V>;
pub type Machine<V> = kirin_interpreter_2::effect::Stateless<V>;
```

Then delete the old machine module and remove its exports.

- [ ] **Step 3: Implement `ResolveCall` for function call statements**

In `lifted.rs`, implement:
- `ResolveCall<'ir, I>` for `Call<T>`
- `Interpretable<'ir, I>` for `Call<T>` using:

```rust
let args = interp.read_many(self.args())?;
let callee = self.resolve_call(interp, &args)?;
interp.invoke(callee, &args, self.results())?;
Ok(Effect::Stay)
```

The resolution logic should use the shell-facing lookup chain, not a function-local runtime trait.

- [ ] **Step 4: Rewrite `Return<T>` against `Invoke::return_current`**

Use:

```rust
let value = <I as ValueStore>::Value::new_product(interp.read_many(&self.values)?);
interp.return_current(value)
```

`Return` should no longer touch any call-frame machine directly.

- [ ] **Step 5: Run the function crate tests**

Run: `cargo test -p kirin-function interpreter2 -- --nocapture`

Expected: PASS for same-stage call, recursion, and multi-result tests.

- [ ] **Step 6: Commit the function migration**

```bash
git add crates/kirin-function/src/interpreter2 crates/kirin-function/src/lib.rs
git commit -m "refactor(function): use shell-owned invocation in interpreter2"
```

### Task 6: Move specialization lookup from function runtime into `SingleStage`

**Files:**
- Modify: `crates/kirin-interpreter-2/src/interpreter/single_stage/frame_ops.rs`
- Modify: `crates/kirin-interpreter-2/src/interpreter/invoke.rs`
- Modify: `crates/kirin-function/src/interpreter2/lifted.rs`
- Modify: `crates/kirin-function/src/interpreter_support.rs`
- Test: `crates/kirin-function/src/interpreter2/tests/programs.rs`

- [ ] **Step 1: Add shell-private lookup helpers**

Implement private `SingleStage` helpers for:
- `lookup_function(symbol)`
- `lookup_staged(function, stage_id)`
- `lookup_specialization(staged, args)`
- `entry_block(callee)`

Start with the current single-specialization policy from `kirin-function/src/interpreter_support.rs`.

- [ ] **Step 2: Narrow `interpreter_support.rs`**

Keep only helpers that still belong to `kirin-function` as IR structure helpers. Move any runtime lookup or specialization-selection code that now belongs to `SingleStage` out of `kirin-function`.

- [ ] **Step 3: Make `Call<T>::resolve_call` use the shell lookup surface**

Do not hard-code `Symbol -> single live specialization` in the statement body. The statement should request resolution from the shell so future `StaticCall`, overloads, and stage-switching calls can share the same runtime path.

- [ ] **Step 4: Run targeted tests**

Run:
- `cargo test -p kirin-function interpreter2::tests::call_resumes_in_caller`
- `cargo test -p kirin-function interpreter2::tests::recursive_counter`
- `cargo test -p kirin-function interpreter2::tests::multi_return_packs_top_level_stop`

Expected: PASS

- [ ] **Step 5: Commit the lookup move**

```bash
git add crates/kirin-interpreter-2/src/interpreter/single_stage crates/kirin-function/src/interpreter_support.rs crates/kirin-function/src/interpreter2/lifted.rs
git commit -m "refactor(interpreter): move lifted call lookup into single-stage shell"
```

---

## Chunk 3: Cleanup, Docs, and Full Verification

### Task 7: Repair remaining dialect integrations and tests

**Files:**
- Modify: `crates/kirin-cf/src/interpreter2/interpret.rs`
- Modify: `crates/kirin-cf/src/interpreter2/mod.rs`
- Modify: `crates/kirin-interpreter-2/src/tests/composite_exec.rs`
- Modify: `crates/kirin-function/src/interpreter2/tests/harness.rs`
- Modify: `crates/kirin-function/src/interpreter2/tests/mod.rs`

- [ ] **Step 1: Remove imports that referenced deleted traits**

Update `kirin-cf` and the interpreter test harness to stop importing `ShellState`, `ValueBindings`, or `ResumeSeed`. If block binding is still needed outside `SingleStage`, keep it through `BlockBindings<'ir>`.

- [ ] **Step 2: Rework test harnesses to use `Invoke`**

Update tests and helpers so they call:
- `ResolveCall::resolve_call`
- `Invoke::invoke`
- `Invoke::return_current`

Do not reintroduce raw frame or binding swap access in tests.

- [ ] **Step 3: Run focused crate tests**

Run:
- `cargo test -p kirin-interpreter-2`
- `cargo test -p kirin-cf`
- `cargo test -p kirin-function`

Expected: PASS

- [ ] **Step 4: Commit the integration cleanup**

```bash
git add crates/kirin-cf/src/interpreter2 crates/kirin-interpreter-2/src/tests crates/kirin-function/src/interpreter2/tests
git commit -m "test(interpreter): update interpreter2 integrations for invoke API"
```

### Task 8: Sync docs and run full verification

**Files:**
- Modify: `docs/design/2026-03-24-kirin-interpreter-machine/index.md`
- Modify: `docs/design/2026-03-24-kirin-interpreter-machine/interpreter-shells.md`
- Modify: `docs/design/2026-03-24-kirin-interpreter-machine/machine.md`
- Modify: `docs/plans/2026-03-25-single-stage-activation-stack-refactor-plan.md`

- [ ] **Step 1: Verify the checked-in design docs still match the implementation**

Confirm the code matches the approved design points:
- `SingleStage` owns the activation stack
- call request types implement `ResolveCall`
- `SingleStage` implements `Invoke`
- `Function`/`StagedFunction`/`SpecializedFunction` remain the semantic owners of function identity and specialization structure

If the implementation diverged, update the design docs before claiming completion.

- [ ] **Step 2: Run formatting**

Run: `cargo fmt --all`

Expected: no diff after formatting

- [ ] **Step 3: Run full targeted verification**

Run:
- `cargo test -p kirin-interpreter-2`
- `cargo test -p kirin-function`
- `cargo test -p kirin-cf`
- `cargo clippy -p kirin-interpreter-2 -p kirin-function -p kirin-cf --all-targets -- -D warnings`

Expected: all PASS

- [ ] **Step 4: Commit the finalized refactor**

```bash
git add docs/design/2026-03-24-kirin-interpreter-machine docs/plans/2026-03-25-single-stage-activation-stack-refactor-plan.md
git commit -m "refactor(interpreter): move single-stage invocation to shell activation stack"
```
