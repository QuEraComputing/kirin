+++
rfc = "0010"
title = "interpreter framework improvements"
status = "Implemented"
agents = ["claude"]
authors = ["Roger-luo <code@rogerluo.dev>"]
created = "2026-02-13T03:31:57.631283Z"
last_updated = "2026-02-13T12:00:00.000000Z"
+++

# RFC 0010: interpreter framework improvements

## Summary

Merge `Session` into `StackInterpreter`, slim the `Interpreter` trait to a minimal read/write contract, eliminate panics from execution methods, and add resource limits (fuel + stack depth) and `Debug` derives. This restructuring makes the interpreter the single concept for "the thing that interprets" — different impls mean different walking strategies.

## Motivation

- Problem: The original framework had three concepts for what should be one: `Interpreter` (state bag trait), `Session` (execution driver), and `StackInterpreter` (concrete state bag). This forced users to juggle `session.interpreter_mut()` for frame access and `session.run()` for execution. The `Interpreter` trait also leaked frame management methods (`push_call_frame`, `pop_call_frame`, `current_frame`) that only make sense for a stack-based strategy.
- Problem: `Session` contained `panic!`, `expect()`, and `unreachable!` in execution paths. No resource bounding existed.
- Why now: These must be fixed before downstream users build on the framework.
- Stakeholders: `kirin-interpreter`, all downstream dialect crates.

## Goals

- Merge `Session` into `StackInterpreter`. Delete `Session` entirely.
- Slim `Interpreter` trait to 3 methods: `read_ref`, `read` (default), `write`.
- Move frame management off the trait into `StackInterpreter` inherent methods.
- Zero `panic!`/`expect`/`unreachable!` in execution methods (`step`, `advance`, `run`, `call`).
- Configurable fuel and stack depth limits.
- `Debug` on `Frame`, `ExecutionControl`.
- Deduplicate block-argument binding with `push_call_frame_with_args` helper.
- Fix `resolve_entry` to use iterator `.next()` instead of collecting to `Vec`.

## Non-goals

- `AbstractInterpreter` / `FixpointDriver` for abstract interpretation (see RFC 0011).
- Derive macro for `Interpretable`.
- `SmallVec` for `ExecutionControl` arguments (separate PR).
- Built-in `Interpretable` impls for `kirin-cf`/`kirin-function`.

## Guide-level Explanation

### Before: Three concepts

```rust
// Old: Interpreter = state bag, Session = execution driver
let interp = StackInterpreter::new(|| Error::NoFrame, Error::UnboundValue);
let mut session = Session::new(interp, &pipeline, stage_id);
let result = session.call::<MyDialect>(func, &[10, 3])?;
// Need session.interpreter_mut() to access frames
```

### After: One concept

```rust
// New: StackInterpreter IS the interpreter — it owns state AND drives execution
// Uses the default InterpError type — no error type annotation needed:
let mut interp: StackInterpreter<i64, _> = StackInterpreter::new(&pipeline, stage_id);
let result = interp.call::<MyDialect>(func, &[10, 3])?;
// Direct frame access: interp.push_call_frame(frame)

// With global state (type-changing builder, G inferred):
let mut interp = StackInterpreter::<i64, _>::new(&pipeline, stage)
    .with_global(my_state)
    .with_fuel(10_000)
    .with_max_depth(256);
```

Error handling uses the `InterpreterError` trait instead of function pointers:

```rust
pub trait InterpreterError {
    fn no_frame() -> Self;
    fn unbound_value(value: SSAValue) -> Self;
}
```

A default `InterpError` enum is provided covering both error conditions. Custom error types can implement the trait for richer diagnostics.

The `Interpreter` trait is now minimal — just what `Interpretable<I>` implementations need:

```rust
pub trait Interpreter {
    type Value;
    type Error;
    fn read_ref(&self, value: SSAValue) -> Result<&Self::Value, Self::Error>;
    fn read(&self, value: SSAValue) -> Result<Self::Value, Self::Error>
    where Self::Value: Clone { self.read_ref(value).cloned() }
    fn write(&mut self, result: ResultValue, value: Self::Value) -> Result<(), Self::Error>;
}
```

Frame management (`push_call_frame`, `pop_call_frame`, `current_frame`) is **off the trait** — these are `StackInterpreter`-specific. A future `AbstractInterpreter` would manage state differently (block-level maps, not a call stack).

Resource limits via builder methods:

```rust
let mut interp: StackInterpreter<i64, _> =
    StackInterpreter::new(&pipeline, stage)
        .with_fuel(10_000)
        .with_max_depth(256);
```

## Reference-level Explanation

### API changes

#### 1. Interpreter trait: slimmed to 3 methods

```rust
pub trait Interpreter {
    type Value;
    type Error;
    fn read_ref(&self, value: SSAValue) -> Result<&Self::Value, Self::Error>;
    fn read(&self, value: SSAValue) -> Result<Self::Value, Self::Error>
    where Self::Value: Clone;
    fn write(&mut self, result: ResultValue, value: Self::Value) -> Result<(), Self::Error>;
}
```

Removed: `current_frame`, `current_frame_mut`, `unbound_value_error`, `push_call_frame`, `pop_call_frame`.

#### 2. InterpreterError trait

```rust
pub trait InterpreterError {
    /// No call frame / fuel exhaustion / unexpected halt / bad state.
    fn no_frame() -> Self;
    /// An SSA value was read before being written.
    fn unbound_value(value: SSAValue) -> Self;
}
```

`StackInterpreter` requires `E: InterpreterError` on impl blocks that produce errors, replacing the previous `fn() -> E` and `fn(SSAValue) -> E` function pointer fields. This eliminates two constructor parameters and makes the error contract explicit.

#### 3. StackInterpreter absorbs Session

```rust
pub struct StackInterpreter<'ir, V, S, E = InterpError, G = ()>
where S: CompileStageInfo
{
    frames: Vec<Frame<V>>,
    global: G,
    pipeline: &'ir Pipeline<S>,
    active_stage: CompileStage,
    breakpoints: HashSet<Statement>,
    fuel: Option<u64>,
    max_depth: Option<usize>,
    _error: PhantomData<E>,  // E: InterpreterError bound on impl blocks
}
```

Type parameter order: `V` (value), `S` (stage info), `E` (error, defaults to `InterpError`), `G` (global state, defaults to `()`). The defaults mean most users only specify `<V, S>`:

```rust
// Minimal — uses InterpError and no global state:
let mut interp: StackInterpreter<i64, _> = StackInterpreter::new(&pipeline, stage);

// Custom error type:
let mut interp: StackInterpreter<i64, _, MyError> = StackInterpreter::new(&pipeline, stage);

// Global state via type-changing builder (G inferred from value):
let mut interp = StackInterpreter::<i64, _>::new(&pipeline, stage)
    .with_global(my_state);
```

`with_global` is defined on `StackInterpreter<..., ()>` and returns `StackInterpreter<..., G>`, transforming the global state type. This means `G` is always inferred — users never need to annotate it.

Public execution methods (moved from Session): `step`, `advance`, `run`, `run_until_break`, `call`.

Public frame access (inherent, not on trait): `current_frame`, `current_frame_mut`, `push_call_frame`, `pop_call_frame`.

Public state: `pipeline()`, `global()`, `global_mut()`, `set_breakpoints()`, `clear_breakpoints()`.

Builder: `with_fuel(u64)`, `with_max_depth(usize)`.

#### 4. Session deleted

`session.rs` removed entirely. `lib.rs` no longer exports `Session`.

#### 5. Panic elimination

`E::no_frame()` (from `InterpreterError`) is used for all "interpreter in bad state" errors (no cursor, unexpected halt, fork in call, fuel exhausted, max depth exceeded).

| Location | Before | After |
|---|---|---|
| `step`: `.expect("no current statement")` | panic | `Err(E::no_frame())` |
| `advance`: `.expect("no current statement")` | panic | `Err(E::no_frame())` |
| `call`: `panic!("Halt reached...")` | panic | `Err(E::no_frame())` |
| `call`: `unreachable!(...)` | panic | explicit `Fork`/catch-all arms returning `Err` |

#### 6. Resource limits

- `step()` decrements fuel before executing; returns `Err` when exhausted.
- `push_call_frame()` checks `frames.len() < max_depth`; returns `Err` when exceeded.
- Defaults: `None` (unlimited), preserving current behavior.

#### 7. Debug derives

`#[derive(Debug)]` on `Frame<V>` and `ExecutionControl<V>` (derive macro adds `V: Debug` bound on the generated impl only).

#### 8. Dedup + resolve_entry fix

Extracted `push_call_frame_with_args<L>` helper used by both `advance(Call)` and `call`. `resolve_entry` uses `.next()` on iterators instead of `.collect::<Vec<_>>().first()`.

### Crate impact matrix

| crate | impact | tests to update |
| --- | --- | --- |
| `kirin-interpreter` | All changes in this RFC | All existing tests updated (Session → StackInterpreter) |

## Drawbacks

- **Breaking change**: `Interpreter` trait loses 5 methods. `StackInterpreter` gains lifetime + stage type params and requires `E: InterpreterError`. Only `StackInterpreter` currently implements `Interpreter`, so impact is minimal.
- **`resolve_stage` still uses `.expect()`**: This is a programmer error (wrong stage type), not a runtime condition. Kept as-is.

## Rationale and Alternatives

### Proposed approach

- The interpreter IS the thing that interprets. No separate session.
- Frame management off the trait: `Interpretable` impls never call `push_call_frame`/`pop_call_frame`. Only the interpreter's own execution methods use them. An `AbstractInterpreter` would manage state differently.
- `InterpreterError` trait replaces fn pointers: proper Rust idiom, no constructor parameter explosion, and the error contract is explicit in the type system.

### Alternative: Keep Session, add error methods to Interpreter trait

- Pros: Smaller diff.
- Cons: Perpetuates the conceptual split. Users still juggle two objects.
- Rejected: The whole point is that the interpreter IS the execution driver.

### Alternative: fn pointer params instead of trait

- Description: Store `fn() -> E` and `fn(SSAValue) -> E` in the struct, pass at construction time.
- Pros: No trait bound needed.
- Cons: Two extra constructor params, stored state for what should be a type-level contract, not idiomatic Rust.
- Rejected: `InterpreterError` trait is cleaner — error construction is a property of the error type, not of each interpreter instance.

## Prior Art

- **Wasmtime fuel**: Step-counting mechanism returning trap on exhaustion.
- **MLIR interpreter**: Single interpreter object owns state and drives execution.

## Backward Compatibility and Migration

- Breaking: `Session` deleted. `Interpreter` trait slimmed. `StackInterpreter` signature changed. Error type must implement `InterpreterError`.
- Migration: Implement `InterpreterError` for your error type. Replace `Session::new(interp, &pipeline, stage)` with `StackInterpreter::new(&pipeline, stage)`. Replace `session.call()` with `interp.call()`. Replace `session.interpreter_mut()` with direct `interp` access.
- Since `kirin-interpreter` is 0.1.0, these are semver-compatible.

## Reference Implementation Plan

Implemented in a single PR:
1. Add `InterpreterError` trait. Slim `Interpreter` trait to 3 methods.
2. Expand `StackInterpreter` to absorb Session's fields and methods, using `E: InterpreterError` bounds.
3. Delete `session.rs`, update `lib.rs`.
4. Add `Debug` derives to `Frame` and `ExecutionControl`.
5. Update all tests (implement `InterpreterError`, simplify constructors).

### Acceptance Criteria

- [x] `Session` type deleted entirely
- [x] `Interpreter` trait has only 3 methods (read_ref, read, write)
- [x] Zero `panic!`/`expect`/`unreachable!` in execution methods
- [x] Fuel and max_depth builder methods available
- [x] `Debug` on `Frame` and `ExecutionControl`
- [x] `resolve_entry` uses `.next()` instead of collecting
- [x] `push_call_frame_with_args` deduplicates argument binding
- [x] All 14 existing tests pass
- [x] Full workspace passes

## Revision Log

| Date | Change |
| --- | --- |
| 2026-02-13 | RFC created from multi-perspective review of RFC 0009 implementation |
| 2026-02-13 | Simplified scope: hardening only. Abstract interpretation split to RFC 0011 |
| 2026-02-13 | Rewritten: primary change is Session→StackInterpreter merge. Hardening comes naturally from restructuring. Status: Implemented. |
