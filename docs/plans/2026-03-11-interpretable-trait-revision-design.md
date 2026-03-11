# Interpretable Trait Revision: Move `L` from Trait to Method

**Date:** 2026-03-11
**Status:** Approved

## Problem

The `#[derive(Interpretable)]` macro cannot generate `InnerType: Interpretable<'ir, I, L>`
bounds in the where clause because it triggers E0275 (infinite trait resolution):

```
HighLevel: Interpretable<'ir, I, HighLevel>
  -> FunctionBody: Interpretable<'ir, I, HighLevel>  (where clause bound)
    -> FunctionBody's impl needs L: Interpretable     (for eval_block)
      -> HighLevel: Interpretable<'ir, I, HighLevel>  (cycle)
```

The current workaround requires dialect authors to manually re-state all inner dialect
value bounds via `#[interpret(where(I::Value: Clone + Add<...> + ...))]`, which is
verbose, fragile, and exposes internal type parameter names.

## Root Cause

The cycle exists because `L` is on the **trait** (`Interpretable<'ir, I, L>`). Impl-level
where clause bounds are checked eagerly by the trait solver. When `L = HighLevel` appears
in a recursive type's impl (via `eval_block`), the solver recurses infinitely.

## Solution

Move `L` from the trait parameter to the method:

```rust
// Before
pub trait Interpretable<'ir, I: Interpreter<'ir>, L: Dialect> {
    fn interpret(&self, interp: &mut I)
        -> Result<Continuation<I::Value, I::Ext>, I::Error>;
}

// After
pub trait Interpretable<'ir, I: Interpreter<'ir>> {
    fn interpret<L: Dialect>(&self, interp: &mut I)
        -> Result<Continuation<I::Value, I::Ext>, I::Error>
    where
        I::StageInfo: HasStageInfo<L>,
        I::Error: From<InterpreterError>,
        L: Interpretable<'ir, I> + 'ir;
}
```

This breaks the cycle because:

1. **Impl-level bounds** (`InnerType: Interpretable<'ir, I>`) only require value-level
   bounds (e.g., `I::Value: Clone + Add`). No `L`, no recursion.
2. **Method-level bounds** (`L: Interpretable<'ir, I>`) are resolved coinductively
   (lazily) by the trait solver since we're already inside the impl being checked.

## Derive Output

The derive generates `InnerType: Interpretable<'__ir, I>` bounds automatically from
`#[wraps]` variants. No user annotation needed:

```rust
#[derive(Interpretable)]
pub enum HighLevel {
    #[wraps] Lexical(Lexical<ArithType>),
    #[wraps] Arith(Arith<ArithType>),
    // ...
}

// Generates:
impl<'__ir, I> Interpretable<'__ir, I> for HighLevel
where
    I: Interpreter<'__ir>,
    Lexical<ArithType>: Interpretable<'__ir, I>,
    Arith<ArithType>: Interpretable<'__ir, I>,
    // ...
```

`#[interpret(where(...))]` is removed entirely.

## CallSemantics

Same pattern applied to `CallSemantics`:

```rust
// Before: CallSemantics<'ir, I, L>
// After:  CallSemantics<'ir, I>, with L on eval_call method
```

## Migration Scope

### kirin-interpreter (trait definitions)
- `Interpretable<'ir, I, L>` -> `Interpretable<'ir, I>`, `L` to method
- `CallSemantics<'ir, I, L>` -> `CallSemantics<'ir, I>`, `L` to method
- `BlockEvaluator::eval_block` — `L: Interpretable<'ir, Self, L>` -> `L: Interpretable<'ir, Self>`
- `Staged` builder methods
- Blanket `CallSemantics` impls for `SSACFGRegion` types
- `StackInterpreter` and `AbstractInterpreter` dispatch/frame logic

### kirin-derive-interpreter (derive macros)
- `interpretable.rs` — generate `InnerType: Interpretable<'__ir, I>` bounds, delete `parse_interpret_where`
- `eval_call/generate.rs` — same for `CallSemantics`
- Snapshot tests

### Dialect crates (7 crates, ~15 impls)
- kirin-arith, kirin-cf, kirin-cmp, kirin-bitwise, kirin-constant, kirin-function, kirin-scf
- Drop `L` from trait params, add method-level where clause
- Delete convenience traits added during prior iteration (ArithInterp, CfInterp, etc.)

### Consumers
- `example/toy-lang/src/language.rs` — remove `#[interpret(where(...))]`
- `crates/kirin-test-languages/src/composite_language.rs` — same
- `crates/kirin-interpreter/tests/*.rs` (4 files) — same

### Docs
- AGENTS.md interpreter conventions
- MEMORY.md
