# MVP Implementation Report

**Date:** 2026-03-31
**Status:** single-stage block execution working, 6 tests passing

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
| `concrete.rs` | `SingleStage<'ir, L, V>` — the concrete interpreter |

## Design Decisions With Rationale

### 1. `Machine::Effect = ()` on the interpreter

The interpreter itself produces no effects. Dialect effects are handled in
`step()` via `IntoAction<V>`, not through `Machine::consume_effect`. This keeps
the interpreter's Machine impl trivial while the effect dispatch happens where
it's needed.

**Potential generalization:** `Machine::Effect` could become the top-level
composed effect type when we add effect delegation to inner dialect machines.
The interpreter's `consume_effect` would then dispatch to inner machines for
dialect-specific effects and handle cursor/frame effects directly.

### 2. `IntoAction<V>` instead of direct marker trait dispatch

The `step()` method uses `IntoAction<V>` to convert dialect effects into
`Action::Advance` or `Action::Jump`. This is narrower than checking all marker
traits at runtime. `()` maps to `Advance`, `CursorEffect<V>` maps directly.

**Potential generalization:** `IntoAction` could check marker traits internally,
or the interpreter could use marker traits directly for richer effect
classification (Call, Return, Yield, Stop). The marker traits exist in
`effect.rs` already; the question is when they become the primary dispatch
surface.

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

### 5. Unit effect `()` means "no effect", default to advance

When `Interpretable::Effect = ()`, the dialect produced no effect. The
interpreter's default policy is to advance the cursor. This is implemented via
`IntoAction for ()` mapping to `Action::Advance`.

This differs from the initial design where `()` was purely "no effect" with no
advance semantics. In practice, advancing is always the right default when a
dialect has nothing to say about control flow (arith, constant, bitwise, cmp).

**Potential generalization:** if a dialect truly wants "no effect and no
advance" (e.g., `Stay` semantics), it should use an explicit effect type with a
`Stay` variant. The unit convention is intentionally opinionated for the common
case.

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
- 4 unit tests on `Frame<V, X>` (read, write, into_parts, cursor methods).

## Deferred Work

### Near-term (next iteration)

- **Multi-block CFG execution**: region traversal where Jump effects follow
  successor blocks. Requires either a `RegionCursor` or the interpreter
  handling `Action::Jump` by replacing the `BlockCursor` on the current frame.
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
