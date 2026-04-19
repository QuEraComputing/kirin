# Interpreter-9 Design Principles

## Goal

Interpreter-9 is a redesigned interpreter framework that eliminates the
semantic friction from interpreter-8. The three core changes from interpreter-8:

1. A **mode discriminant** (`Env::type Mode`) that enables coherent split impls
   for operations that behave differently in concrete vs. abstract mode.
2. **Inbox-based yield threading** — a `StackEntry<C, V>.inbox` field replaces
   the old `pending_yield` side channel.
3. **`Interpretable<E>` replaces `Semantics<D>`** — a single trait with a
   single return type `Control<E::Value, E::Ext>`, no associated `Effect` type.

---

## Core Types

### `Env` — the unified domain trait

```rust
pub trait Env {
    type Mode;          // ConcreteMode<C> or AbstractMode<C>
    type Value: Clone;
    type Ext;           // CursorExt<C> in practice
    type Error: From<InterpreterError>;
    type Stages: StageMeta;
    // read, write_result, write_ssa, write_results, ...
}
```

`type Mode` is the key discriminant. It carries the cursor type `C` as a
phantom so that split impls on `E: Env<Mode = ConcreteMode<C>>` and
`E: Env<Mode = AbstractMode<C>>` are coherent — Rust sees them as targeting
different associated types and accepts both.

### `Interpretable<E>` — the dialect op trait

```rust
pub trait Interpretable<E: Env> {
    fn eval(&self, env: &mut E) -> Result<Control<E::Value, E::Ext>, E::Error>;
}
```

No associated type. Pure ops (Arith, Cmp, Bitwise, Constant) implement this
with a single `impl<E: Env> Interpretable<E>` that works for both modes.
Mode-specific ops (SCF If/For) use two impls bounded by `E::Mode`.

### `Control<V, Ext>` — the effect type

```
Advance         — step to next statement
Jump(Block, args) — unconditional branch (intra-function)
Fork(branches)  — nondeterministic branch (abstract only)
Return(V)       — exit current function frame
Yield(V)        — pass value to parent cursor's inbox
Call { callee, stage, args, results } — cross-function call
Ext(Ext)        — push/pop a cursor (CursorExt::Push / Pop)
```

### `StackEntry<C, V>` — inbox-bearing cursor entry

```rust
pub struct StackEntry<C, V> {
    pub cursor: C,
    pub inbox: Option<V>,
}
```

When a child cursor completes with `Control::Yield(v)`, the driver deposits `v`
into the parent's `inbox`. The parent cursor reads `inbox` on its next
`execute()` call. This eliminates the old `pending_yield` side channel — the
yield value travels through the cursor stack, not through a mutable field on
the interpreter.

---

## Cursor Architecture

### Framework cursors (live in `kirin-interpreter-9`)

| Type | Purpose |
|------|---------|
| `BlockCursor<V, L>` | Concrete-mode linear scan of a single block |
| `AbstractBlockCursor<V, L>` | Abstract-mode block scan; on Jump/Fork calls `enqueue_block` and returns `Pop` |

These are the only cursor types the framework defines. They are generic over
the dialect language `L` and value type `V`.

**Why two separate types?** Rust's coherence checker (E0119) rejects two `impl
Execute<E> for BlockCursor<V, L>` blocks that differ only in `E::Mode`. Making
`AbstractBlockCursor` a distinct type gives each its own impl with no conflict.

### Dialect cursors (live in the dialect crate)

SCF cursors (`IfCursor`, `ForCursor`, `AbstractIfCursor`, `AbstractForCursor`,
`SCFCursor`, `AbstractSCFCursor`) live in `kirin-scf/src/interpreter9/cursor.rs`,
**not** in the framework. The framework has no knowledge of structured control
flow; it only provides the cursor-stack driver.

The `ForLoopValue` trait also lives in `kirin-scf`, alongside its `i64` impl.

This separation means:
- Adding a new dialect with cursor-based semantics (e.g., a region dialect)
  does not require touching `kirin-interpreter-9`.
- Dialect crate authors can define their own cursor coproducts without coupling
  to unrelated dialect concepts.

### Cursor coproduct (live in the composed language)

A composed language like `toy-lang` defines:

```rust
pub enum HighLevelCursor<V> {
    Block(BlockCursor<V, HighLevel>),
    Scf(SCFCursor<V, HighLevel>),
}
```

with `Lift` impls for each variant and a single `Execute<E>` impl that
dispatches to the inner cursor. This is mechanical glue; a future
`#[derive(ComposedCursor)]` could generate it.

---

## Concrete vs. Abstract Execution

### Concrete (`ConcreteInterp`)

- `cursors: Vec<StackEntry<C, V>>` — the cursor stack.
- `step()` pops the top cursor, calls `execute(env, inbox)`, then handles the
  returned `Control` variant:
  - `Advance` / `Jump` — cursor continues (re-pushed).
  - `Ext(Push(new))` — push the new cursor on top, re-push current.
  - `Ext(Pop)` — cursor is done, discard.
  - `Yield(v)` — deposit `v` into the parent cursor's inbox (or set `result`
    if the stack is empty).
  - `Return(v)` — pop the frame; write `v` to caller results or set `result`.
  - `Call { ... }` — push a new frame + cursor for the callee.
  - `Fork` — error; concrete mode cannot handle nondeterminism.

### Abstract (`AbstractInterp`)

- `cursor_stack: Vec<StackEntry<C, V>>` — for SCF cursor traversal.
- `func_worklist` / `block_worklist` — the worklist fixpoint engine.
- `block_in: HashMap<Block, Vec<V>>` — abstract block entry states.
- `summaries: HashMap<SpecializedFunction, FuncSummary>` — return values.

For flat CF (no SCF): the cursor stack is always empty; `AbstractBlockCursor`
handles `Jump` by enqueuing the target block and returning `Pop`. The driver
just pops the worklist and creates a new `AbstractBlockCursor` for each block.

For SCF: `AbstractIfCursor` / `AbstractForCursor` use the cursor stack to
analyze both branches / loop iterations abstractly, joining results via
`AbstractValue::join` / widening.

---

## Dialect Composition Contract

A dialect that participates in interpreter-9 provides:

1. `impl<E: Env> Interpretable<E> for Op` for pure ops (works in both modes).
2. Mode-split helper functions (e.g., `eval_if_concrete`, `eval_if_abstract`)
   for ops that require different cursor construction per mode.
3. Cursor types in the dialect crate for any ops that span multiple `execute()`
   calls (e.g., `IfCursor`, `ForCursor`).

The composed language wires these together in its own `Interpretable<E>` impl,
matching on each variant and calling the appropriate helper or `op.eval(env)`.

---

## Design Tensions and Tradeoffs

| Tension | Resolution |
|---------|-----------|
| Coherence for mode-split impls | Two separate cursor types instead of two impls on the same type |
| `ForLoopValue` in framework vs. dialect | Trait lives in `kirin-scf`; framework knows nothing about it |
| SCF cursors in framework vs. dialect | All SCF cursors in `kirin-scf/src/interpreter9/`; framework only has `BlockCursor` |
| Inbox vs. side-channel for yield | `StackEntry.inbox` field — yield flows through the cursor stack, not a mutable interpreter field |
| Abstract SCF fixpoint | `AbstractIfCursor` joins both branch yields; `AbstractForCursor` iterates with widening up to `max_iterations` |
