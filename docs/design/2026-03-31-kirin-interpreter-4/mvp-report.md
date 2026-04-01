# MVP Implementation Report

**Date:** 2026-04-01 (updated)
**Status:** execution seeds implemented, 10 tests passing

## Module Structure

| File | Purpose |
|------|---------|
| `traits.rs` | `Machine`, `ValueStore`, `PipelineAccess`, `Interpretable<I>`, `Interpreter` (blanket) |
| `effect.rs` | Marker traits (`IsAdvance`, `IsJump`, `IsCall`, `IsReturn`, `IsYield`) + `CursorEffect<V>` |
| `lift.rs` | `Lift`/`Project`/`ProjectRef`/`ProjectMut` + `LiftInto`/`TryLiftInto` |
| `error.rs` | `InterpreterError` (9 variants) |
| `frame.rs` | `Frame<V, X>` with SSA value bindings |
| `frame_stack.rs` | `FrameStack<V, X>` with max-depth enforcement |
| `cursor.rs` | `BlockCursor` for linear block traversal |
| `concrete.rs` | `SingleStage<'ir, L, V, M>`, `Action<V, R>`, `Lift` impls |

## Design Decisions With Rationale

### 1. `Action<V, R>` is the interpreter's effect algebra

The interpreter's `Machine::Effect = Action<V, M::Effect>`. Dialect effects are
lifted into `Action` via the `Lift` trait, then consumed by the interpreter's
`consume_effect`:

- `Action::Advance` → advance cursor
- `Action::Jump(block, args)` → jump cursor to block
- `Action::Delegate(inner)` → delegate to inner machine's `consume_effect`

This was reached in two steps. The initial MVP used `Machine::Effect = ()` with
a separate `IntoAction` trait to convert dialect effects into cursor actions.
The generalization recognized that `IntoAction` was just `Lift` under a
different name, and `Action` was already the interpreter's natural effect type.
Merging them:

- Eliminated `IntoAction` entirely
- Removed the `E` type parameter from `SingleStage` (the effect type is derived
  as `Action<V, M::Effect>`)
- Unified effect conversion under the existing `Lift`/`LiftInto` infrastructure

**Why this works without trait overlap:** The identity blanket
`impl<T> Lift<T> for T` only applies when `Self = From`. Our impls
`Lift<()> for Action<V, R>` and `Lift<CursorEffect<V>> for Action<V, R>` have
distinct `Self` and `From` types, so no conflict.

### 2. Inner dialect machine via `M` parameter

`SingleStage<'ir, L, V, M>` has an inner machine `M` (default `()`) accessible
via `machine()` / `machine_mut()`. Dialect authors mutate it during `interpret`.
Effects the interpreter can't handle are delegated via `Action::Delegate`.

The constraint `M: Machine<Effect = R, Error = InterpreterError>` ties the
inner machine's effect type to `Action`'s `R` parameter. For `M = ()`,
`R = ()` and `Delegate(())` is a no-op.

Composite machines implement `ProjectRef<Sub>` and `ProjectMut<Sub>` to expose
sub-machines. `SingleStage` forwards this via `project_machine::<Sub>()` and
`project_machine_mut::<Sub>()`. Identity impls ensure every machine can project
to itself, so single-machine interpreters work without extra boilerplate.

### 3. `Interpreter` trait has no methods

It's a blanket supertrait of `Machine + ValueStore + PipelineAccess`. This
means dialect authors bound on `I: Interpreter` and get all three capabilities.
No methods of its own.

**Potential generalization:** execution seed methods (`exec_block`, `invoke`)
could become provided methods on `Interpreter` or a separate `ExecSeed` trait.
This would let dialect authors call execution seeds via the trait bound rather
than knowing the concrete interpreter type.

### 4. `exec_block` takes a `SpecializedFunction`

For the MVP, `exec_block` reuses `enter_function` (push frame, run, pop). A
true "inline block execution" without frames would be a narrower primitive.

**Potential generalization:** separate `exec_block` (cursor-only push/pop
within the same frame, no new frame) from `invoke` (push a new frame for a
function invocation). The frame-less version is what `scf.if` and `scf.for`
actually need.

### 5. Unit effect `()` lifts to `Action::Advance`

When `Interpretable::Effect = ()`, the dialect produced no effect. Via
`Lift<()> for Action<V, R>`, this lifts to `Action::Advance` — the interpreter
advances the cursor.

This is intentionally opinionated: no effect means advance. If a dialect wants
"no cursor movement" (Stay semantics), it should use an explicit effect type
with a Stay variant.

### 6. Test uses local `TestDialect` with `#[derive(Dialect)]`

The orphan rule prevents implementing `Interpretable` for `CompositeLanguage`
in the test crate (neither trait nor type is local). The test defines a local
`TestDialect` enum using `#[derive(Dialect)]` with `#[wraps]`, which delegates
all `Dialect` supertraits to the inner types. The `Interpretable` impl is then
local to the test.

This pattern will be standard for interpreter-4 tests: define a local dialect
enum wrapping the operations you need, implement `Interpretable` on it.

## Test Coverage

- `test_constant_and_run` — builds a one-block function (`%0 = constant 42;
  ret %0`), runs with `run()`, verifies the constant value in the SSA store.
- `test_step_by_step` — same program driven by `step()`: first step executes
  constant, second executes return terminator, third returns `false` (exhausted).
- `test_counter_machine` — `CounterMachine` tracks statement count via
  `interp.machine_mut()` inside `interpret`. Verifies `count == 2` after
  running constant + return.
- `test_composite_machine_projection` — `CompositeMachine` with two
  sub-machines (`CounterMachine`, `TraceMachine`). Dialect projects to each
  via `interp.project_machine_mut::<Sub>()`. Verifies counter and trace
  independently.
- 4 unit tests on `Frame<V, X>` (read, write, into_parts, cursor methods).

## Execution Seeds Implementation (2026-04-01)

Replaced the per-frame `BlockCursor` with user-definable cursor types and a
global cursor stack. Key changes from the MVP:

| Before | After |
|--------|-------|
| `Action<V, R>` (3 variants) | `Action<V, R, C>` (7 variants: +Return, Yield, Push, Call) |
| `Frame<V, X>` with per-frame cursor | `Frame<V>` with `caller_results`, no cursor |
| `FrameStack<V, X>` | `FrameStack<V>` |
| `SingleStage<'ir, L, V, M>` | `SingleStage<'ir, L, V, M, C>` with `cursors: Vec<C>` |
| `step()` interprets one statement | `step()` pops cursor, calls `execute`, dispatches effect |
| `run()` returns `()` | `run()` returns `Option<V>` (top-level return value) |
| `BlockCursor` (not generic) | `BlockCursor<V>` with `Execute<SingleStage<...>>` impl |

### Design decisions

1. **Global cursor stack** (not per-frame): Per-frame cursor stacks break when
   cursors cross frame boundaries. The global stack naturally mirrors nesting.

2. **Call as a driver-handled effect**: The frame stack is interpreter-internal
   state. Cursors return `Call` effects; the driver handles frame push/pop.
   Eliminates `FunctionCursor` for standard calls.

3. **`C: Lift<BlockCursor<V>>` on the driver**: The `Call` handler creates a
   `BlockCursor` for the callee's entry block and lifts it into `C`. All cursor
   entry types must accept `BlockCursor<V>` as a variant.

4. **Block exhaustion = error**: Well-formed IR blocks end with a terminator
   producing a structural effect (Return, Yield, Call, Push). Block exhaustion
   without one returns `InterpreterError::NoCurrent`.

5. **Top-level Return → `pending_yield`**: When Return pops the last frame,
   the value is stored in `pending_yield`. `run()` returns it.

### Test coverage (10 tests)

- 4 frame unit tests (read, write, into_parts, caller_results)
- `test_constant_and_run` — basic block execution with Return
- `test_step_by_step` — driver step semantics
- `test_counter_machine` — dialect machine mutation during interpret
- `test_composite_machine_projection` — ProjectRef/ProjectMut for sub-machines
- `test_push_inline_block` — Push effect for inline block execution with Yield
- `test_jump_multi_block` — Jump effect for multi-block CFG traversal

## Deferred Work

### Near-term

- **`RegionCursor<V>`**: CFG traversal with proper region semantics (entry
  block, follow jumps, return on Return/Yield).
- **`ForCursor<V>`**: Loop iteration via Push/Yield cycle with the driver.
- **Synchronous execution seed methods**: `exec_block`, `exec_region`, `invoke`
  as convenience wrappers around the cursor stack.
- **Callee query builder**: Port `callee::Query` from interpreter-2.

### Medium-term

- **`Receipt` trait**: Bundle `Language`, `Value`, `Machine`, `CursorEntry`,
  `StageInfo`, `Error`.
- **`#[derive(Execute)]`**: Auto-generate delegating impls for composite cursor
  enums.
- **`IsPush` marker trait usage**: Defined but not yet used by the driver (it
  matches `Action` directly).
- **Generalize cursor type visibility**: Add `type CursorEntry` to `Interpreter`
  so dialects producing Push effects can write generic impls.
- **Driver control traits**: `Fuel`, `Breakpoints`, `Interrupt`.
- **`Position` trait**: Read-only cursor inspection.

### Long-term

- **Abstract interpreter**: Fixpoint cursor types, summary computation.
- **Derive macros**: `#[derive(Interpretable)]`, `#[derive(Machine)]`.
- **Graph cursors**: `DiGraphCursor`, `UnGraphCursor`.
- **Custom calling conventions**: Coroutines, generators, graph functions.
- **Dynamic interpreter**: Multi-stage with heterogeneous value types.
