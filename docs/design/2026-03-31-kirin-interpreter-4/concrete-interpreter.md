# Concrete Interpreter

## Current Shape: `SingleStage`

```rust
pub struct SingleStage<'ir, L: Dialect, V: Clone, M = (), C = BlockCursor<V>> {
    pipeline: &'ir Pipeline<StageInfo<L>>,
    stage_id: CompileStage,
    frames: FrameStack<V>,
    cursors: Vec<C>,
    machine: M,
    pending_yield: Option<V>,
}
```

Five type parameters:
- `L` — dialect (language enum)
- `V` — value type
- `M` — inner dialect machine (default `()`)
- `C` — cursor entry type (default `BlockCursor<V>`)

`Machine::Effect = Action<V, M::Effect, C>`.

## Interpreter Trait Stack

```rust
trait ValueStore { type Value; type Error; fn read/write/write_ssa... }
trait PipelineAccess { type StageInfo; fn pipeline/current_stage... }
trait Machine { type Effect; type Error; fn consume_effect... }
trait Interpreter: Machine + ValueStore + PipelineAccess {}  // blanket
```

`Interpreter` is a blanket supertrait — no methods of its own.

## Frame and FrameStack

```rust
pub struct Frame<V> {
    callee: SpecializedFunction,
    stage: CompileStage,
    values: FxHashMap<SSAValue, V>,
    caller_results: Vec<ResultValue>,
}
```

No generic extra-state parameter. The `caller_results` field records where the
driver should write the return value when this frame is popped. For the
top-level frame, `caller_results` is empty and the return value is captured in
`pending_yield`.

`FrameStack<V>` is a `Vec<Frame<V>>` with max-depth enforcement and
ValueStore-like delegating helpers.

## Global Cursor Stack

The cursor stack is a single `Vec<C>` on the interpreter, **not** per-frame.
This is the key architectural decision — per-frame cursor stacks break when
cursors cross frame boundaries (e.g., Call handling).

The cursor stack naturally mirrors nesting:
```
[0] BlockCursor(main)       ← paused at call site
[1] BlockCursor(callee)     ← running callee's entry block
```

Return consumes the callee's cursor. The parent's cursor sits below.

## Driver Loop

The driver pops a cursor entry, calls `Execute::execute`, and dispatches the
returned `Action` directly via match:

```rust
pub fn step(&mut self) -> Result<bool, InterpreterError> {
    let Some(mut entry) = self.cursors.pop() else {
        return Ok(false);
    };
    let effect = entry.execute(self)?;
    match effect {
        Action::Push(new) => { push entry, push new }
        Action::Yield(v)  => { pending_yield = Some(v) }
        Action::Return(v) => { pop frame, write to caller_results }
        Action::Call(...)  => { push entry, push_call_frame }
        Action::Advance    => { push entry back }
        Action::Jump(..)   => { push entry back }
        Action::Delegate(r)=> { machine.consume_effect(r), push entry }
    }
    Ok(true)
}

pub fn run(&mut self) -> Result<Option<V>, InterpreterError> {
    while self.step()? {}
    Ok(self.pending_yield.take())
}
```

**Pop-before-execute:** The entry is popped before `execute` to avoid borrow
conflicts — `entry` (owned) and `interp` (`&mut self`) are separate objects.

**Top-level Return:** When Return pops the last frame, the value is stored in
`pending_yield` (no caller frame to write to). `run()` returns it.

**Driver constraint:** `C: Execute<Self> + Lift<BlockCursor<V>>`. The `Lift`
bound is required because Call handling creates a `BlockCursor` for the callee's
entry block and lifts it into `C`.

## Machine Impl

`SingleStage`'s `consume_effect` only handles `Delegate` — all structural
effects are handled by the driver loop, not by `consume_effect`:

```rust
fn consume_effect(&mut self, effect: Action<V, M::Effect, C>) -> Result<(), Error> {
    match effect {
        Action::Delegate(inner) => self.machine.consume_effect(inner),
        _ => Err(InterpreterError::UnhandledEffect(...)),
    }
}
```

## Receipt Trait (Deferred)

A `Receipt` trait bundling `Language`, `Value`, `Machine`, `CursorEntry`,
`StageInfo`, `Error` would simplify the 5-parameter generics. Deferred until
patterns stabilize.

## Deferred Topics

- `Receipt` trait for type parameter bundling
- `AbstractInterpreter` with fixpoint execution seeds
- Dynamic interpreter (multi-stage with heterogeneous value types)
- Driver control traits (Fuel, Breakpoints, Interrupt)
- `Position` trait for read-only cursor inspection
- Stage dispatch cache for multi-dialect interpreters
