# Change Continuation::Jump to use Block instead of Successor

**Date:** 2026-03-02
**Status:** Approved
**Scope:** kirin-interpreter, kirin-cf, kirin-scf, kirin-function

## Motivation

`Continuation::Jump(Successor, Args<V>)` requires dialect interpreter impls that work with owned body blocks (e.g., `scf::If`) to convert `Block → Successor` via `Successor::from_block()` just to satisfy the type signature. Meanwhile, the interpreter internals immediately call `succ.target()` to get back to `Block`. The `Successor` wrapper adds no value at the interpreter level — it's an IR-level concept (control-flow edge annotation) that doesn't belong in the runtime continuation type.

## Design

### Change

```rust
// Before (kirin-interpreter/src/control.rs)
enum Continuation<V, Ext = Infallible> {
    Jump(Successor, Args<V>),
    Fork(SmallVec<[(Successor, Args<V>); 2]>),
    // ...
}

// After
enum Continuation<V, Ext = Infallible> {
    Jump(Block, Args<V>),
    Fork(SmallVec<[(Block, Args<V>); 2]>),
    // ...
}
```

### What stays the same

- `Successor` type in kirin-ir (newtype over Id, `target()`, `from_block()`)
- Dialect op struct definitions (`Branch { target: Successor, args: Vec<SSAValue> }`)
- `HasSuccessors` / `HasSuccessorsMut` traits
- Arena structure (no new arenas)

### Impact on dialect interpreter impls

**kirin-cf** — add `.target()` when constructing Jump from Successor fields:
```rust
// Branch
Ok(Continuation::Jump(target.target(), values))
// ConditionalBranch
Ok(Continuation::Jump(true_target.target(), t_values))
```

**kirin-scf** — remove `Successor::from_block()` conversions:
```rust
// If (simpler)
Ok(Continuation::Jump(self.then_body, smallvec![]))
```

**kirin-function** — if any impls use Successor, add `.target()`.

### Impact on interpreter internals

- `stack/transition.rs`: Remove `succ.target()` calls — already a Block
- `abstract_interp/fixpoint.rs`: Remove `succ.target()` calls
- `stack/call.rs`: Remove `succ.target()` calls

### Import changes

- `kirin-interpreter/src/control.rs`: Import `Block` instead of `Successor`
- Dialect interpret_impl files: Adjust imports as needed
