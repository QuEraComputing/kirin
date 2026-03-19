# Simplify Interpretable Trait Bounds

## Problem

Every manual `Interpretable` impl repeats the same four-line where clause:

```rust
fn interpret<L>(&self, interp: &mut I) -> Result<Continuation<I::Value, I::Ext>, I::Error>
where
    I::StageInfo: HasStageInfo<L>,
    I::Error: From<InterpreterError>,
    L: Interpretable<'ir, I> + 'ir,
```

This pattern appears in every dialect: `Arith`, `Bitwise`, `Cmp`, `FunctionBody`, `Lambda`, `Call`, `Return`, `Bind`, `Lexical`, `Lifted`, `If`, `For`, `Yield`, `Branch`, `CondBranch`, `Constant`. That is at least 16 impls, each repeating the same 3 method-level bounds.

The trait definition itself (`Interpretable<'ir, I: Interpreter<'ir>>`) cannot absorb these bounds because:
- `L` is a method-level generic (not a trait parameter) to break the E0275 cycle
- The bounds reference `L`, so they must be on the method

This plan explores trait-level simplification that does NOT rely on macros (per user request).

## Research Findings

### The three method bounds and why each exists

1. **`I::StageInfo: HasStageInfo<L>`** -- Required so the interpreter can resolve `StageInfo<L>` from its type-erased `StageInfo`. Used by `interp.resolve_stage::<L>()` and `interp.active_stage_info::<L>()`.

2. **`I::Error: From<InterpreterError>`** -- Required so `InterpreterError` (the framework error) can be converted to the interpreter's concrete error type via `?`. Every impl uses `?` on operations that return `InterpreterError`.

3. **`L: Interpretable<'ir, I> + 'ir`** -- Required for coinductive trait resolution. When a dialect delegates to an inner type (`inner.interpret::<L>(interp)`), the compiler needs to know `L` itself implements `Interpretable`. The `'ir` bound ensures the language enum lives long enough.

### Can any bounds move to the trait definition?

The trait is:
```rust
pub trait Interpretable<'ir, I: Interpreter<'ir>>: Dialect {
    fn interpret<L>(&self, interpreter: &mut I) -> Result<...>
    where
        I::StageInfo: HasStageInfo<L>,
        I::Error: From<InterpreterError>,
        L: Interpretable<'ir, I> + 'ir;
}
```

**`I::Error: From<InterpreterError>`** -- This bound does NOT depend on `L`. It could move to the trait level:

```rust
pub trait Interpretable<'ir, I: Interpreter<'ir>>: Dialect
where
    I::Error: From<InterpreterError>,
{
    fn interpret<L>(&self, interpreter: &mut I) -> Result<...>
    where
        I::StageInfo: HasStageInfo<L>,
        L: Interpretable<'ir, I> + 'ir;
}
```

**Impact:** This removes one bound from every method signature. However, it adds the bound to every `impl` block's where clause (since `impl Interpretable<'ir, I> for T where I::Error: From<InterpreterError>`). Net effect: the bound moves from the method to the trait/impl, but doesn't disappear. However, it becomes "pay once at impl level" rather than "repeat on every method call site".

**Verdict:** Marginal improvement. The bound on the trait is slightly cleaner but shifts complexity rather than removing it.

### Can a supertrait bundle common bounds?

Idea: create a trait alias or supertrait that bundles the bounds.

```rust
pub trait InterpretableWith<'ir, I: Interpreter<'ir>>:
    Interpretable<'ir, I> + 'ir
where
    I::StageInfo: HasStageInfo<Self>,
    I::Error: From<InterpreterError>,
{}
```

Then method bounds become `L: InterpretableWith<'ir, I>`.

**Problem:** Rust trait aliases are unstable. A supertrait with where clauses requires the bounds to be satisfied at the impl site, which is the same situation we have now.

However, we CAN define a helper trait that bundles the method-level bounds:

```rust
/// Bundle of bounds needed for dialect interpretation dispatch.
pub trait LanguageBounds<'ir, I: Interpreter<'ir>>:
    Interpretable<'ir, I> + 'ir
{}

impl<'ir, I, L> LanguageBounds<'ir, I> for L
where
    I: Interpreter<'ir>,
    L: Interpretable<'ir, I> + 'ir,
{}
```

Then methods become:
```rust
fn interpret<L>(&self, interp: &mut I) -> Result<...>
where
    I::StageInfo: HasStageInfo<L>,
    I::Error: From<InterpreterError>,
    L: LanguageBounds<'ir, I>;
```

This only saves the `+ 'ir` -- not worth a new trait.

### Can associated type defaults help?

No. The bounds are on method parameters (`L`) and associated types of `I`, not on associated types of `Interpretable`.

### The real bottleneck: `L` on the method

The fundamental constraint is that `L` must be a method-level generic. If `L` were a trait parameter, the bounds could live on the impl. But `L` on the trait caused E0275 (infinite recursion during trait solving).

With `L` on the method, every call site must specify all three bounds. This is inherent to the design and cannot be simplified at the trait level without changing the `L` placement.

### Alternative: move `From<InterpreterError>` to `Interpreter` supertrait

The `Interpreter` trait is a blanket supertrait of `BlockEvaluator`. We could add `Error: From<InterpreterError>` as a bound on `BlockEvaluator` (or `ValueStore`):

```rust
pub trait ValueStore {
    type Value: ...;
    type Error: From<InterpreterError>;  // <-- add this bound
    // ...
}
```

Currently, `ValueStore` has no bound on `Error`. Adding `From<InterpreterError>` would:
- Remove the need for `I::Error: From<InterpreterError>` on every `interpret` method
- But constrain ALL interpreter implementations to have `Error: From<InterpreterError>`

**Analysis of existing interpreters:**
- `StackInterpreter<'ir, V, S, E>` has `E = InterpreterError` by default. `InterpreterError: From<InterpreterError>` is trivially satisfied.
- `AbstractInterpreter<'ir, V, S, E, G>` has a generic `E`. Users must provide `E: From<InterpreterError>`.

This is a reasonable constraint -- any interpreter that runs dialect code MUST be able to handle `InterpreterError` variants (like `UnboundValue`, `FuelExhausted`). Making this a supertrait bound is semantically correct and eliminates one of the three method bounds everywhere.

### Proposed: move `From<InterpreterError>` to `ValueStore::Error`

**Before (every interpret method):**
```rust
fn interpret<L>(&self, interp: &mut I) -> Result<...>
where
    I::StageInfo: HasStageInfo<L>,
    I::Error: From<InterpreterError>,  // this line removed
    L: Interpretable<'ir, I> + 'ir,
```

**After:**
```rust
fn interpret<L>(&self, interp: &mut I) -> Result<...>
where
    I::StageInfo: HasStageInfo<L>,
    L: Interpretable<'ir, I> + 'ir,
```

This removes ~16 lines across manual dialect impls and simplifies every call site.

## Proposed Design

### Change 1: Add `From<InterpreterError>` bound to `ValueStore::Error`

In `kirin-interpreter/src/value_store.rs` (or wherever `ValueStore` is defined):

```rust
pub trait ValueStore {
    type Value;
    type Error: From<InterpreterError>;  // was: type Error;
    // ...
}
```

### Change 2: Remove `I::Error: From<InterpreterError>` from `Interpretable` method

```rust
pub trait Interpretable<'ir, I: Interpreter<'ir>>: Dialect {
    fn interpret<L>(&self, interpreter: &mut I) -> Result<Continuation<I::Value, I::Ext>, I::Error>
    where
        I::StageInfo: HasStageInfo<L>,
        L: Interpretable<'ir, I> + 'ir;
        // I::Error: From<InterpreterError> removed -- guaranteed by ValueStore
}
```

### Change 3: Remove from `CallSemantics` method

Same change for `CallSemantics::eval_call<L>`.

### Change 4: Update derive macro

In `kirin-derive-interpreter/src/interpretable.rs`, remove the `I::Error: From<InterpreterError>` predicate from the generated `method_where_clause`.

### Change 5: Update all manual dialect impls

Remove `I::Error: From<InterpreterError>` from the where clause of every `interpret<L>` method across:
- `kirin-arith/src/interpret_impl.rs`
- `kirin-bitwise/src/interpret_impl.rs`
- `kirin-cmp/src/interpret_impl.rs`
- `kirin-function/src/interpret_impl.rs`
- `kirin-scf/src/interpret_impl.rs`
- `kirin-cf/src/interpret_impl.rs`
- `kirin-constant/src/interpret_impl.rs`

### Non-change: keep `I::StageInfo: HasStageInfo<L>` and `L: Interpretable<'ir, I> + 'ir`

These cannot be simplified further:
- `HasStageInfo<L>` depends on `L` (method-level generic)
- `L: Interpretable<'ir, I> + 'ir` is needed for coinductive resolution

## Implementation Steps

1. **Add `From<InterpreterError>` bound** to `ValueStore::Error` associated type.
2. **Verify `StackInterpreter` and `AbstractInterpreter`** satisfy the new bound (they should, since they already require it in practice).
3. **Remove `I::Error: From<InterpreterError>`** from `Interpretable::interpret` method signature.
4. **Remove from `CallSemantics::eval_call`** method signature.
5. **Update derive macro** in `kirin-derive-interpreter/src/interpretable.rs` to stop generating this bound.
6. **Update all manual dialect impls** (7 crates) to remove the bound from `interpret<L>` methods.
7. **Run full test suite**: `cargo nextest run --workspace && cargo test --doc --workspace`.

## Risk Assessment

**Low risk:**
- Every existing interpreter already satisfies `Error: From<InterpreterError>` in practice -- they use `?` on `InterpreterError`-producing operations. This just makes an implicit requirement explicit.
- The change is purely subtractive at call sites (removing a where clause bound).

**Potential breakage:**
- If anyone has a custom `ValueStore` implementation where `Error` does NOT implement `From<InterpreterError>`, this would break. This is unlikely because such an interpreter could not use any of the built-in dialect impls.
- If it does happen, the fix is trivial: add `impl From<InterpreterError> for MyError`.

**Migration path:**
- This is a breaking change to the `ValueStore` trait. Semver bump needed if kirin follows semver. Since kirin is pre-1.0, this is acceptable.

## Testing Strategy

- **Existing tests pass**: The full test suite (`cargo nextest run --workspace`) must pass after the change. No new tests needed -- this is a bound simplification, not a behavior change.
- **Verify derive output**: Update snapshot tests in `kirin-derive-interpreter` to reflect the removed bound.
- **Manual review**: Grep for `From<InterpreterError>` in method where clauses to ensure all instances are removed.
