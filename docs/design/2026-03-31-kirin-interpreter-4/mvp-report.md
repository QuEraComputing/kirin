# MVP Implementation Report

**Date:** 2026-03-31
**Status:** single-stage block execution working, 7 tests passing

## Module Structure

| File | Purpose |
|------|---------|
| `traits.rs` | `Machine`, `ValueStore`, `PipelineAccess`, `Interpretable<I>`, `Interpreter` (blanket) |
| `effect.rs` | Marker traits (`IsAdvance`, `IsJump`, `IsCall`, `IsReturn`, `IsYield`) + `CursorEffect<V>` |
| `lift.rs` | `Lift`/`Project`/`TryLift`/`TryProject` + `LiftInto`/`TryLiftInto` |
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

**Potential generalization:** `Project<Sub>`/`ProjectMut<Sub>` for composite
machines, letting dialect authors project to specific sub-machines instead of
accessing the whole machine.

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
- 4 unit tests on `Frame<V, X>` (read, write, into_parts, cursor methods).

## Deferred Work

### Near-term (next iteration)

- **Multi-block CFG execution**: region traversal where Jump effects follow
  successor blocks. The interpreter already handles `Action::Jump` by replacing
  the `BlockCursor` on the current frame — this may already work for intra-region
  jumps.
- **Function invocation**: `invoke(callee, args) -> Result<V, E>` as an
  execution seed. Push a new frame, execute the callee's entry region, pop,
  return the result.
- **`ExecSeed` trait on `Interpreter`**: move `exec_block` and `invoke` to a
  trait so dialect authors can call them via `I: Interpreter + ExecSeed`
  rather than knowing the concrete type.

### Medium-term

- **`Receipt` trait**: bundle `Language`, `Value`, `Machine`, `StageInfo`,
  `Error` into one associated-type carrier. Simplifies `SingleStage` generics.
- **Dialect machine composition**: `Project<Sub>`/`ProjectMut<Sub>` for
  composite machines. The interpreter forwards projection to give dialect
  authors `&mut SubMachine` during `interpret`.
- **Driver control traits**: `Fuel`, `Breakpoints`, `Interrupt` — carried
  forward from interpreter-2 design unchanged.
- **`Position` trait**: read-only cursor inspection for tests and debugging.

### Long-term

- **Abstract interpreter**: `AbstractInterpreter<R>` with fixpoint execution
  seeds. `exec_block` runs to fixpoint, `invoke` checks/computes summaries.
- **Derive macros**: `#[derive(Interpretable)]`, `#[derive(Machine)]` for
  composite dialect/machine/effect enums.
- **Graph execution seeds**: DiGraph, UnGraph traversal.
- **Dynamic interpreter**: multi-stage with heterogeneous value types.
- **Stage-boundary protocols**: cross-stage execution orchestration.
