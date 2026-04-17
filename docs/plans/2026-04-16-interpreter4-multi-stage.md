# interpreter-4 Multi-Stage Generalization

## Goal

Generalize `SingleStage` to a `MultiStage<S: StageMeta>` interpreter that executes programs
spanning multiple dialects/stages. Dialect authors write `Interpretable<I>` once; the same
impl drives both `SingleStage` and `MultiStage`. Write integration tests covering all dialect
crates and cross-stage execution.

## Design

### Key insight: PipelineAccess::current_stage_info<L>

Add a provided method to `PipelineAccess` that replaces the `SingleStage`-specific `stage_info()`:

```rust
fn current_stage_info<L>(&self) -> Option<&StageInfo<L>>
where
    Self::StageInfo: HasStageInfo<L>,
    L: Dialect,
{
    self.pipeline().stage(self.current_stage()).and_then(|s| s.try_stage_info())
}
```

`StageInfo<L>: HasStageInfo<L>` returns `Some(self)`, so `SingleStage` works as before.
`S: HasStageInfo<L>` dispatches dynamically for `MultiStage`.

### BlockCursor<V, L>

Add `L: Dialect` as phantom type. `Execute<I>` impl uses
`interp.current_stage_info::<L>()` — works for both `SingleStage<L>` and `MultiStage<S>`
as long as `I::StageInfo: HasStageInfo<L>`.

```rust
pub struct BlockCursor<V, L: Dialect> {
    block: Block,
    current: Option<Statement>,
    results: Vec<ResultValue>,
    args: Option<Vec<V>>,
    _phantom: PhantomData<L>,
}
```

### MultiStage<'ir, S, V, M>

```rust
pub struct MultiStage<'ir, S: StageMeta, V: Clone, M = ()> {
    pipeline: &'ir Pipeline<S>,
    root_stage: CompileStage,
    frames: FrameStack<V>,
    cursors: Vec<Box<dyn Execute<Self> + 'ir>>,
    machine: M,
    pending_yield: Option<V>,
}
```

Cursor type is `Box<dyn Execute<Self> + 'ir>` — heterogeneous via trait objects.
`Execute<I>` is object-safe (`fn execute(&mut self, interp: &mut I) -> Result<I::Effect, I::Error>`).

`Machine::Effect` for `MultiStage` = `Action<V, M::Effect, Box<dyn Execute<Self> + 'ir>>`.

`enter_function<L>`: creates `Box::new(BlockCursor::<V, L>::new(...))`.
`push_call_frame`: uses `CallPayload.callee_stage` + `StageDispatch` to dispatch to the right `L`.

### CallPayload + kirin-function::Call

```rust
pub struct CallPayload<V> {
    pub callee: SpecializedFunction,
    pub callee_stage: CompileStage,   // NEW
    pub args: Vec<V>,
    pub results: Vec<ResultValue>,
}
```

`Call::interpret` generalized from `I::StageInfo = StageInfo<L>` to `I::StageInfo: HasStageInfo<L>`.
Uses `interp.current_stage_info::<L>()`. Includes `callee_stage: interp.current_stage()` in payload.

### IfCursor / ForCursor — add body_stage

```rust
pub struct IfCursor<V> {
    phase: IfPhase<V>,
    body_stage: CompileStage,   // NEW — set from interp.current_stage() at If execute time
}
```

`scf::If::interpret` stores `interp.current_stage()` in the `IfCursor` it creates.

`Execute<MultiStage<S>>` for `IfCursor<V>`: uses `StageDispatch` with a `MakeCursorAction` to
dynamically create `Box<dyn Execute<MultiStage<S>>>` for the body block:

```rust
struct MakeCursorAction<'ir, S, V, M> {
    block: Block, args: Vec<V>, results: Vec<ResultValue>,
    output: Option<Box<dyn Execute<MultiStage<'ir, S, V, M>> + 'ir>>,
}

impl<S, L, V, M> StageAction<S, L> for MakeCursorAction<'_, S, V, M>
where ..., BlockCursor<V, L>: Execute<MultiStage<'_, S, V, M>>
{
    fn run(&mut self, _stage_id, stage_info: &StageInfo<L>) -> Result<(), Error> {
        self.output = Some(Box::new(BlockCursor::<V, L>::new(stage_info, self.block, ...)));
        Ok(())
    }
}
```

`Execute<SingleStage<L>>` for `IfCursor<V>` is updated to use `BlockCursor<V, L>` (L known from impl).

## Milestones

### M1 — interpreter-4 core + kirin-function (parallel with M2)

**Files: kirin-interpreter-4/src/traits.rs, cursor.rs, concrete.rs, effect.rs**
**Files: kirin-function/src/interpreter4/interpret.rs**

1. `PipelineAccess`: add `current_stage_info<L>` provided method
2. `BlockCursor<V>` → `BlockCursor<V, L>`: add `L: Dialect` phantom, drop `stage_info()` calls
3. Update `Execute<SingleStage<L>>` impl for `BlockCursor<V, L>`
4. `CallPayload<V>`: add `callee_stage: CompileStage` field
5. Update `Lift<CallPayload<V>>` for `Action<V, R, C>` (no-op, just struct field)
6. `kirin-function`'s `Call::interpret`: generalize bounds, add `callee_stage`
7. Update existing tests in interpreter-4 (`BlockCursor<V>` → `BlockCursor<V, TestDialect>`)

### M2 — MultiStage struct + driver (parallel with M1)

**Files: kirin-interpreter-4/src/concrete.rs (new section), lib.rs**

1. Add `MultiStage<'ir, S, V, M>` struct
2. Implement `Machine`, `ValueStore`, `PipelineAccess` for `MultiStage`
3. Add `enter_function<L>()` method (creates boxed `BlockCursor<V, L>`)
4. Add `step()` and `run()` driver loop (same logic as `SingleStage`, cursor type is boxed)
5. Add `push_call_frame` using `StageDispatch` + `callee_stage` from `CallPayload`
6. Export from `lib.rs`

Depends on M1 for `BlockCursor<V, L>` and `CallPayload.callee_stage`.

### M3 — SCF cursors (depends on M1+M2)

**Files: kirin-scf/src/interpreter4/cursor.rs, kirin-scf/src/interpreter4/interpret.rs**

1. Add `body_stage: CompileStage` to `IfCursor<V>` and `ForCursor<V>`
2. Update constructors: pass `interp.current_stage()` from `If::interpret` / `For::interpret`
3. Update `Execute<SingleStage<L>>` for `IfCursor`/`ForCursor` to use `BlockCursor<V, L>`
4. Add `MakeCursorAction<'ir, S, V, M>` struct + `StageAction` impl
5. Add `Execute<MultiStage<S>>` for `IfCursor<V>` using `MakeCursorAction`
6. Add `Execute<MultiStage<S>>` for `ForCursor<V>` using `MakeCursorAction`
7. Add `Lift<IfCursor<V>>` + `Lift<ForCursor<V>>` for `Box<dyn Execute<MultiStage<S>>>`

### M4 — Integration tests (depends on M2+M3)

**Files: kirin-interpreter-4/tests/multi_stage.rs**
**Files: kirin-test-languages/src/lib.rs (new multi-stage test language)**

1. Define two-stage test language: `StageA` (Constant+Arith+CF+Function) + `StageB` (same)
2. Test: same-stage call (equivalent to SingleStage)
3. Test: cross-stage call (main at StageA, helper at StageB)
4. Test: SCF with MultiStage (if + for loop within a stage)
5. Test: all dialect ops covered (arith, bitwise, cmp, cf, scf, constant, function)

### M5 — toy-lang multi-stage e2e (depends on M4)

**Files: example/toy-lang/tests/e2e.rs or new multi_stage_e2e.rs**

1. Implement `Interpretable<MultiStage<Stage>>` for `HighLevel` and `LowLevel`
2. Port existing toy-lang tests (add, factorial, branching) to use `MultiStage`
3. Add a cross-stage test: main in `Source`, helper in `Lowered`
4. Verify output matches old `StackInterpreter` results

## Cross-cutting concerns

- No boilerplate: `Interpretable<I>` impls in dialect crates stay unchanged (generic over I).
  Only kirin-function and kirin-scf cursor internals change.
- Composibility: `SingleStage` still works for all existing code. `MultiStage` is additive.
- Extensibility: new dialects just implement `Interpretable<I>` once; works for both shells.
