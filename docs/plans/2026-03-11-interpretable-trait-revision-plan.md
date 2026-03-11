# Interpretable Trait Revision: Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Move `L` (language type parameter) from trait-level to method-level on `Interpretable` and `CallSemantics`, breaking the E0275 cycle so `#[derive(Interpretable)]` auto-generates inner-type bounds without `#[interpret(where(...))]`.

**Architecture:** The core change is `Interpretable<'ir, I, L>` → `Interpretable<'ir, I>` with `L` becoming a generic on the `interpret` method. Same for `CallSemantics`. This breaks the recursive trait resolution cycle because impl-level bounds (`InnerType: Interpretable<'ir, I>`) only require value-level bounds (no `L`, no recursion), while method-level bounds (`L: Interpretable<'ir, I>`) are resolved coinductively.

**Tech Stack:** Rust, proc-macro derive infrastructure (syn/quote/darling), insta snapshot tests

---

### Task 1: Revise `Interpretable` trait definition

**Files:**
- Modify: `crates/kirin-interpreter/src/interpretable.rs`

**Step 1: Edit the trait**

Change the trait from:
```rust
pub trait Interpretable<'ir, I, L: Dialect>: Dialect
where
    I: Interpreter<'ir>,
{
    fn interpret(&self, interpreter: &mut I) -> Result<Continuation<I::Value, I::Ext>, I::Error>;
}
```

To:
```rust
pub trait Interpretable<'ir, I: Interpreter<'ir>>: Dialect {
    fn interpret<L: Dialect>(
        &self,
        interpreter: &mut I,
    ) -> Result<Continuation<I::Value, I::Ext>, I::Error>
    where
        I::StageInfo: HasStageInfo<L>,
        I::Error: From<InterpreterError>,
        L: Interpretable<'ir, I> + 'ir;
}
```

Note: The method-level where clause includes `L: Interpretable<'ir, I> + 'ir` which is what breaks the cycle. Many dialect impls don't actually need these method bounds (they don't call `eval_block` or access stage info via `L`), but adding them uniformly keeps the trait simple and lets delegation (match arm forwarding) work without worrying about which inner type needs what.

**Step 2: Verify it compiles in isolation**

Run: `cargo check -p kirin-interpreter 2>&1 | head -50`
Expected: Many errors from downstream code still using old signature — this is expected.

**Step 3: Commit**

```
git add crates/kirin-interpreter/src/interpretable.rs
git commit -m "refactor(interpreter): move L from Interpretable trait to method"
```

---

### Task 2: Revise `CallSemantics` trait and `SSACFGRegion` blanket impls

**Files:**
- Modify: `crates/kirin-interpreter/src/call.rs`

**Step 1: Change `CallSemantics` trait**

From:
```rust
pub trait CallSemantics<'ir, I: Interpreter<'ir>, L: Dialect>: Dialect {
    type Result;
    fn eval_call(
        &self,
        interpreter: &mut I,
        stage: &'ir StageInfo<L>,
        callee: SpecializedFunction,
        args: &[I::Value],
    ) -> Result<Self::Result, I::Error>;
}
```

To:
```rust
pub trait CallSemantics<'ir, I: Interpreter<'ir>>: Dialect {
    type Result;
    fn eval_call<L: Dialect>(
        &self,
        interpreter: &mut I,
        stage: &'ir StageInfo<L>,
        callee: SpecializedFunction,
        args: &[I::Value],
    ) -> Result<Self::Result, I::Error>
    where
        I::StageInfo: HasStageInfo<L>,
        I::Error: From<InterpreterError>,
        L: Interpretable<'ir, I> + CallSemantics<'ir, I, Result = Self::Result> + 'ir;
}
```

**Step 2: Update StackInterpreter blanket impl**

From:
```rust
impl<'ir, V, S, E, G, L, T> CallSemantics<'ir, crate::StackInterpreter<'ir, V, S, E, G>, L> for T
where
    T: SSACFGRegion,
    ...
    L: Dialect + crate::Interpretable<'ir, crate::StackInterpreter<'ir, V, S, E, G>, L> + 'ir,
```

To:
```rust
impl<'ir, V, S, E, G, T> CallSemantics<'ir, crate::StackInterpreter<'ir, V, S, E, G>> for T
where
    T: SSACFGRegion,
    V: Clone + 'ir,
    E: From<InterpreterError> + 'ir,
    S: StageMeta + 'ir,
    G: 'ir,
{
    type Result = V;

    fn eval_call<L: Dialect>(
        &self,
        interp: &mut crate::StackInterpreter<'ir, V, S, E, G>,
        stage: &'ir StageInfo<L>,
        callee: SpecializedFunction,
        args: &[V],
    ) -> Result<V, E>
    where
        I::StageInfo: HasStageInfo<L>,  // use S: HasStageInfo<L> (since S is the StageInfo type)
        I::Error: From<InterpreterError>,
        L: Interpretable<'ir, crate::StackInterpreter<'ir, V, S, E, G>>
            + CallSemantics<'ir, crate::StackInterpreter<'ir, V, S, E, G>, Result = V>
            + 'ir,
    {
        // body unchanged
    }
}
```

Wait — the blanket impl needs careful attention. Since we're implementing `CallSemantics<'ir, StackInterpreter<...>>` for `T: SSACFGRegion`, the `L` is now on the method, not the impl. The impl no longer needs `L` at all. The method's where clause has `S: HasStageInfo<L>` (from the concrete `S` type).

Let me rewrite this more precisely. The blanket impl becomes:

```rust
impl<'ir, V, S, E, G, T> CallSemantics<'ir, crate::StackInterpreter<'ir, V, S, E, G>> for T
where
    T: SSACFGRegion,
    V: Clone + 'ir,
    E: From<InterpreterError> + 'ir,
    S: StageMeta + 'ir,
    G: 'ir,
{
    type Result = V;

    fn eval_call<L: Dialect>(
        &self,
        interp: &mut crate::StackInterpreter<'ir, V, S, E, G>,
        stage: &'ir StageInfo<L>,
        callee: SpecializedFunction,
        args: &[V],
    ) -> Result<V, E>
    where
        S: HasStageInfo<L>,
        E: From<InterpreterError>,
        L: Interpretable<'ir, crate::StackInterpreter<'ir, V, S, E, G>>
            + CallSemantics<'ir, crate::StackInterpreter<'ir, V, S, E, G>, Result = V>
            + 'ir,
    {
        let entry = self.entry_block::<L>(stage)?;
        let first = entry.first_statement(stage);
        let frame_stage = interp.resolve_stage_id(stage);
        interp.push_frame(crate::Frame::new(callee, frame_stage, first))?;
        interp.bind_block_args(stage, entry, args)?;
        let initial_depth = interp.frame_depth();
        interp.run_nested_calls(|interp, _is_yield| interp.frame_depth() < initial_depth)
    }
}
```

**Step 3: Update AbstractInterpreter blanket impl (same pattern)**

Same structure — drop `L` from impl generics, add to method where clause.

**Step 4: Commit**

```
git add crates/kirin-interpreter/src/call.rs
git commit -m "refactor(interpreter): move L from CallSemantics trait to method"
```

---

### Task 3: Update `BlockEvaluator::eval_block`

**Files:**
- Modify: `crates/kirin-interpreter/src/block_eval.rs`

**Step 1: Change eval_block where clause**

Line 72: `L: crate::Interpretable<'ir, Self, L>` → `L: crate::Interpretable<'ir, Self>`

The full method signature becomes:
```rust
fn eval_block<L: Dialect>(
    &mut self,
    stage: &'ir StageInfo<L>,
    block: Block,
) -> Result<Continuation<Self::Value, Self::Ext>, Self::Error>
where
    Self::StageInfo: HasStageInfo<L>,
    L: crate::Interpretable<'ir, Self>;
```

**Step 2: Commit**

```
git add crates/kirin-interpreter/src/block_eval.rs
git commit -m "refactor(interpreter): update eval_block to use revised Interpretable"
```

---

### Task 4: Update StackInterpreter dispatch, stage, call, and transition

**Files:**
- Modify: `crates/kirin-interpreter/src/stack/dispatch.rs`
- Modify: `crates/kirin-interpreter/src/stack/stage.rs`
- Modify: `crates/kirin-interpreter/src/stack/call.rs`
- Modify: `crates/kirin-interpreter/src/stack/transition.rs`

**Step 1: dispatch.rs — drop `, L` from all `Interpretable` and `CallSemantics` bounds**

Every occurrence of `Interpretable<'ir, StackInterpreter<...>, L>` becomes `Interpretable<'ir, StackInterpreter<...>>`.
Every occurrence of `CallSemantics<'ir, StackInterpreter<...>, L, Result = V>` becomes `CallSemantics<'ir, StackInterpreter<...>, Result = V>`.

Affected: `CallDynAction` StageAction impl (line 54), `dyn_step_for_lang` (line 78), `dyn_push_call_frame_for_lang` (line 91), `dyn_advance_for_lang` (line 108), `FrameDispatchAction` StageAction impl (line 132).

**Step 2: stage.rs — drop `, L` from Interpretable/CallSemantics bounds**

Line 14: `L: Dialect + Interpretable<'ir, StackInterpreter<...>, L> + 'ir` → `L: Dialect + Interpretable<'ir, StackInterpreter<...>> + 'ir`
Line 42: `L: CallSemantics<'ir, StackInterpreter<...>, L, Result = V>` → `L: CallSemantics<'ir, StackInterpreter<...>, Result = V>`

**Step 3: call.rs — drop `, L` from Interpretable/CallSemantics bounds**

Line 47: `L: Dialect + Interpretable<'ir, Self, L> + CallSemantics<'ir, Self, L, Result = V> + 'ir`
→ `L: Dialect + Interpretable<'ir, Self> + CallSemantics<'ir, Self, Result = V> + 'ir`

Line 68: `L: Dialect + Interpretable<'ir, Self, L> + 'ir`
→ `L: Dialect + Interpretable<'ir, Self> + 'ir`

**Step 4: transition.rs — drop `, L` from Interpretable bounds**

Line 68: `L: Dialect + Interpretable<'ir, Self, L> + 'ir` → `L: Dialect + Interpretable<'ir, Self> + 'ir`
Line 84: same pattern

**Step 5: Commit**

```
git add crates/kirin-interpreter/src/stack/
git commit -m "refactor(interpreter): update stack interpreter to use revised traits"
```

---

### Task 5: Update AbstractInterpreter dispatch and fixpoint

**Files:**
- Modify: `crates/kirin-interpreter/src/abstract_interp/stage.rs`
- Modify: `crates/kirin-interpreter/src/abstract_interp/fixpoint.rs`

**Step 1: stage.rs — drop `, L` from bounds**

Lines 17-19: Change `Interpretable<'ir, AbstractInterpreter<...>, L>` → `Interpretable<'ir, AbstractInterpreter<...>>` and same for `CallSemantics`.

**Step 2: fixpoint.rs — drop `, L` from bounds**

`analyze_with_stage_id` (line 53): `Interpretable<'ir, Self, L>` → `Interpretable<'ir, Self>`
Same for `CallSemantics`.

`analyze_in_resolved_stage` (line 70): same.

`run_forward` (line 118): `L: Dialect + Interpretable<'ir, Self, L> + 'ir` → `L: Dialect + Interpretable<'ir, Self> + 'ir`

`AnalyzeDynAction` StageAction impl (line 393-396): drop `, L` from both trait bounds.

**Step 3: Commit**

```
git add crates/kirin-interpreter/src/abstract_interp/
git commit -m "refactor(interpreter): update abstract interpreter to use revised traits"
```

---

### Task 6: Build and verify kirin-interpreter compiles

**Step 1: Check compilation**

Run: `cargo check -p kirin-interpreter`
Expected: Compiles. Downstream crates will still fail.

**Step 2: Commit (if any fixups needed)**

---

### Task 7: Migrate dialect crate Interpretable impls

**Files:**
- Modify: `crates/kirin-arith/src/interpret_impl.rs`
- Modify: `crates/kirin-cf/src/interpret_impl.rs`
- Modify: `crates/kirin-constant/src/interpret_impl.rs`
- Modify: `crates/kirin-bitwise/src/interpret_impl.rs`
- Modify: `crates/kirin-cmp/src/interpret_impl.rs`
- Modify: `crates/kirin-scf/src/interpret_impl.rs`
- Modify: `crates/kirin-function/src/interpret_impl.rs`

**Pattern for all leaf dialect impls (arith, cf, constant, bitwise, cmp):**

Drop `L` from the impl signature and the `Dialect` bound on `L`. Example for `Arith`:

Before:
```rust
impl<'ir, I, L, T> Interpretable<'ir, I, L> for Arith<T>
where
    I: Interpreter<'ir>,
    I::Value: Clone + Add<...> + ...,
    I::Error: From<InterpreterError>,
    L: Dialect,
    T: CompileTimeValue,
```

After:
```rust
impl<'ir, I, T> Interpretable<'ir, I> for Arith<T>
where
    I: Interpreter<'ir>,
    I::Value: Clone + Add<...> + ...,
    I::Error: From<InterpreterError>,
    T: CompileTimeValue,
```

The `interpret` method gains `<L: Dialect>` as a type parameter and the standard method-level where clause. But since these leaf impls don't actually use `L` in their method body, the method signature just needs to match the trait.

```rust
fn interpret<L: Dialect>(
    &self,
    interpreter: &mut I,
) -> Result<Continuation<I::Value, I::Ext>, I::Error>
where
    I::StageInfo: HasStageInfo<L>,
    I::Error: From<InterpreterError>,
    L: Interpretable<'ir, I> + 'ir,
{
    // body unchanged
}
```

**Pattern for scf `For<T>` (uses `eval_block`, needs `L`):**

Before:
```rust
impl<'ir, I, L, T> Interpretable<'ir, I, L> for For<T>
where
    ...
    L: Dialect + Interpretable<'ir, I, L> + 'ir,
```

After:
```rust
impl<'ir, I, T> Interpretable<'ir, I> for For<T>
where
    I: Interpreter<'ir>,
    I::Value: Clone + ForLoopValue,
    I::Error: From<InterpreterError>,
    T: CompileTimeValue,
{
    fn interpret<L: Dialect>(...) -> ...
    where
        I::StageInfo: HasStageInfo<L>,
        I::Error: From<InterpreterError>,
        L: Interpretable<'ir, I> + 'ir,
    {
        // body uses L for active_stage_info::<L>() and eval_block — these work
        // because L: Interpretable<'ir, I> is on the method
    }
}
```

The key difference: `I::StageInfo: HasStageInfo<L>` and `L: Interpretable<'ir, I>` move from the impl where clause to the method where clause. The impl-level bounds only need value bounds (`ForLoopValue`, `Clone`, etc.).

**Pattern for `kirin-function` impls (FunctionBody, Lambda, Lexical, Call, Return, Bind, Lifted):**

Same pattern. Drop `L` from impl, add to method. For `Lexical` and `Lifted` which use UFCS delegation:

Before:
```rust
<FunctionBody<T> as Interpretable<'ir, I, L>>::interpret(op, interp)
```

After:
```rust
<FunctionBody<T> as Interpretable<'ir, I>>::interpret::<L>(op, interp)
```

Wait — actually, since `L` is now on the method, the UFCS call syntax changes. The turbofish goes on `interpret`:
```rust
Interpretable::interpret::<L>(op, interp)
```
But with UFCS trait qualification:
```rust
<FunctionBody<T> as Interpretable<'ir, I>>::interpret::<L>(op, interp)
```

Hmm, but the compiler should be able to infer `L` from the caller's `L` if we just call `op.interpret::<L>(interp)` — since `L` is now a method generic, the method call will carry the turbofish.

Actually the simplest: just call `op.interpret::<L>(interp)` everywhere. The compiler knows the impl because the concrete type is known.

**Pattern for `CallSemantics` (kirin-function SSACFGRegion impls):**

`SSACFGRegion` stays the same (no `Interpretable` bound). The blanket `CallSemantics` impls are in `call.rs` (already handled in Task 2).

**Step 1: Apply pattern to all 7 dialect crates**

**Step 2: Verify compilation**

Run: `cargo check -p kirin-arith -p kirin-cf -p kirin-constant -p kirin-bitwise -p kirin-cmp -p kirin-scf -p kirin-function`

**Step 3: Commit**

```
git add crates/kirin-arith/ crates/kirin-cf/ crates/kirin-constant/ crates/kirin-bitwise/ crates/kirin-cmp/ crates/kirin-scf/ crates/kirin-function/
git commit -m "refactor(dialects): migrate all dialect Interpretable impls to revised trait"
```

---

### Task 8: Rewrite `#[derive(Interpretable)]` macro

**Files:**
- Modify: `crates/kirin-derive-interpreter/src/interpretable.rs`

**Step 1: Update generated code**

The derive currently generates:
```rust
impl<'__ir, __InterpI, __InterpL> Interpretable<'__ir, __InterpI, __InterpL> for MyType
where
    __InterpI: Interpreter<'__ir>,
    __InterpL: Dialect,
    InnerType: Interpretable<'__ir, __InterpI, __InterpL>,
    ...
```

After:
```rust
impl<'__ir, __InterpI> Interpretable<'__ir, __InterpI> for MyType
where
    __InterpI: Interpreter<'__ir>,
    InnerType: Interpretable<'__ir, __InterpI>,
    ...
{
    fn interpret<__InterpL: Dialect>(
        &self,
        interpreter: &mut __InterpI,
    ) -> Result<Continuation<__InterpI::Value, __InterpI::Ext>, __InterpI::Error>
    where
        __InterpI::StageInfo: HasStageInfo<__InterpL>,
        __InterpI::Error: From<InterpreterError>,
        __InterpL: Interpretable<'__ir, __InterpI> + 'ir,
    {
        match self {
            Self::Variant(field_0) => field_0.interpret::<__InterpL>(interpreter),
            ...
        }
    }
}
```

Key changes:
1. Remove `__InterpL` from impl generics and trait generics
2. Change `InnerType: Interpretable<'__ir, __InterpI, __InterpL>` to `InnerType: Interpretable<'__ir, __InterpI>`
3. Add `<__InterpL: Dialect>` to the method signature
4. Add method-level where clause
5. Add turbofish `::<__InterpL>` to inner `interpret` calls
6. Delete `#[interpret(where(...))]` support entirely (the `parse_interpret_where` function and its integration)

Changes to `do_derive_interpretable`:
- `.generics_modifier`: Remove the `__InterpL` generic parameter push
- `.trait_generics`: Change to `<'__ir, __InterpI>` (drop `__InterpL`)
- `.where_clause`: Remove `__InterpL: Dialect` predicate. Keep `__InterpI: Interpreter<'__ir>`. Change wrapper bounds from `Interpretable<'__ir, __InterpI, __InterpL>` to `Interpretable<'__ir, __InterpI>`.
- `.method`: The `interpret` method needs generic params `<__InterpL: ::kirin::ir::Dialect>`, a where clause, and turbofish on inner calls.

The method pattern needs updating — the `MethodSpec` needs to support generics and method-level where clauses. Let me check if `MethodSpec` supports this.

Looking at the current `MethodSpec` struct — it has `name`, `self_arg`, `params`, `return_type`, and `pattern`. No `generics` or `where_clause` fields. We need to either:
(a) Add `method_generics` and `method_where_clause` to `MethodSpec`
(b) Use `Custom::new` to emit the entire method body manually

Option (b) is simpler for now — use `Custom::new` at the top level to generate the whole impl rather than going through the template. But actually, the template handles the impl block structure — we just need the method to have generics.

Simplest approach: add optional `generics` and `where_clause` fields to `MethodSpec`. But that's in `kirin-derive-toolkit` which other derives also use.

Alternative: The `Custom` pattern closure just returns the match arm body. The template wraps it in the method signature. So we need to extend the template to support method-level generics.

Actually, the cleanest approach: add two optional fields to `MethodSpec`:
```rust
pub method_generics: Option<TokenStream>,   // e.g. `<__InterpL: Dialect>`
pub method_where_clause: Option<TokenStream>, // e.g. `where I::StageInfo: HasStageInfo<__InterpL>, ...`
```

**Step 2: Extend `MethodSpec` in kirin-derive-toolkit**

File: `crates/kirin-derive-toolkit/src/template/method_pattern.rs` (or wherever `MethodSpec` lives)

Add two fields with `Option<TokenStream>` defaults. Update the template rendering code to emit them.

**Step 3: Update derive code**

Update `do_derive_interpretable` to:
1. Drop `__InterpL` from generics_modifier
2. Drop `__InterpL` from trait_generics
3. Drop `__InterpL: Dialect` from where_clause, update wrapper bounds
4. Set `method_generics` and `method_where_clause` on the MethodSpec
5. In the Custom pattern, add turbofish: `field_0.interpret::<__InterpL>(interpreter)`

**Step 4: Run snapshot tests**

Run: `cargo nextest run -p kirin-derive-interpreter`
Expected: Snapshot mismatches. Review and accept.

Run: `cargo insta review`

**Step 5: Commit**

```
git add crates/kirin-derive-toolkit/ crates/kirin-derive-interpreter/src/interpretable.rs crates/kirin-derive-interpreter/src/snapshots/
git commit -m "refactor(derive-interpreter): generate revised Interpretable impl without L on trait"
```

---

### Task 9: Rewrite `#[derive(CallSemantics)]` macro

**Files:**
- Modify: `crates/kirin-derive-interpreter/src/eval_call/generate.rs`

**Step 1: Update generated code**

Same pattern as Interpretable. The derive currently generates:
```rust
impl<'__ir, __CallSemI> CallSemantics<'__ir, __CallSemI, FuncOps> for FuncOps
```

After:
```rust
impl<'__ir, __CallSemI> CallSemantics<'__ir, __CallSemI> for FuncOps
where
    __CallSemI: Interpreter<'__ir>,
    __CallSemI::Error: From<InterpreterError>,
    CallOp: CallSemantics<'__ir, __CallSemI>,
{
    type Result = <CallOp as CallSemantics<'__ir, __CallSemI>>::Result;

    fn eval_call<__CallSemL: Dialect>(
        &self,
        interpreter: &mut __CallSemI,
        stage: &'__ir StageInfo<__CallSemL>,
        callee: SpecializedFunction,
        args: &[__CallSemI::Value],
    ) -> Result<Self::Result, __CallSemI::Error>
    where
        __CallSemI::StageInfo: HasStageInfo<__CallSemL>,
        __CallSemI::Error: From<InterpreterError>,
        __CallSemL: Interpretable<'__ir, __CallSemI>
            + CallSemantics<'__ir, __CallSemI, Result = Self::Result>
            + 'ir,
    {
        match self {
            Self::Call(field_0) => field_0.eval_call::<__CallSemL>(interpreter, stage, callee, args),
            Self::Return(_) => Err(InterpreterError::missing_function_entry().into()),
        }
    }
}
```

Key changes:
1. Remove the type name from trait generics (was `CallSemantics<'__ir, __CallSemI, FuncOps>`)
2. Wrapper bounds change from `CallSemantics<'__ir, __CallSemI, FuncOps>` to `CallSemantics<'__ir, __CallSemI>`
3. `stage` parameter type changes from `&'__ir StageInfo<FuncOps>` to `&'__ir StageInfo<__CallSemL>`
4. Add turbofish on forwarded `eval_call` calls
5. Add method generics and where clause

**Step 2: Update `do_derive_eval_call`**

- `.trait_generics`: Change from `<'__ir, __CallSemI, TypeName>` to `<'__ir, __CallSemI>`
- `.where_clause`: Change wrapper bounds to drop type name from `CallSemantics<...>`
- `.method`: Add `method_generics`, `method_where_clause`, update `stage` param type, add turbofish
- `.assoc_type`: Update to reference `CallSemantics<'__ir, __CallSemI>` (no type name)

**Step 3: Run snapshot tests and accept**

Run: `cargo nextest run -p kirin-derive-interpreter`
Run: `cargo insta review`

**Step 4: Commit**

```
git add crates/kirin-derive-interpreter/src/eval_call/
git commit -m "refactor(derive-interpreter): generate revised CallSemantics impl without L on trait"
```

---

### Task 10: Update consumer files — test languages and toy-lang

**Files:**
- Modify: `example/toy-lang/src/interpret.rs`
- Modify: `crates/kirin-test-languages/src/composite_language.rs`
- Modify: `crates/kirin-interpreter/tests/derive_macros.rs`

**Step 1: toy-lang interpret.rs**

Update `HighLevel` and `LowLevel` manual impls. Drop `L` from trait, add to method:

Before:
```rust
impl<'ir, I> Interpretable<'ir, I, HighLevel> for HighLevel
```

After:
```rust
impl<'ir, I> Interpretable<'ir, I> for HighLevel
where
    I: Interpreter<'ir>,
    I::Value: Clone + Add<...> + ...,
    I::Error: From<InterpreterError>,
{
    fn interpret<L: Dialect>(&self, interp: &mut I) -> Result<...>
    where
        I::StageInfo: HasStageInfo<L>,
        I::Error: From<InterpreterError>,
        L: Interpretable<'ir, I> + 'ir,
    {
        match self {
            HighLevel::Lexical(inner) => inner.interpret::<L>(interp),
            ...
        }
    }
}
```

Same for `LowLevel`.

The `SSACFGRegion` impls are unchanged (they don't reference `Interpretable`).

**Step 2: composite_language.rs — remove `#[interpret(where(...))]`**

`CompositeLanguage` uses `#[derive(Interpretable, CallSemantics)]`. After the derive changes in Tasks 8-9, it should just work without `#[interpret(where(...))]`. But wait — `CompositeLanguage` doesn't currently have `#[interpret(where(...))]`. Let me re-check... Looking at the file, it has:
```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, Interpretable, CallSemantics)]
```
No `#[interpret(where(...))]` — good, no changes needed here.

**Step 3: derive_macros.rs — remove `#[interpret(where(...))]`**

The test file has:
```rust
#[interpret(where(
    I::Value: kirin_arith::ArithInterp
        + kirin_cf::CfInterp
        + From<kirin_arith::ArithValue>,
))]
```

Wait, those convenience traits (`ArithInterp`, `CfInterp`) don't exist in the codebase (they were only in the abandoned uncommitted changes). Let me re-check what the actual current test file has...

I already read it — `derive_macros.rs` lines 17-21 reference `kirin_arith::ArithInterp` and `kirin_cf::CfInterp`. But those don't exist in the current committed code... Let me verify.

Actually, looking at the current code on the `rust` branch, these might be existing traits. Let me check.

Actually wait — I read the file at the start and it has:
```rust
#[interpret(where(
    I::Value: kirin_arith::ArithInterp
        + kirin_cf::CfInterp
        + From<kirin_arith::ArithValue>,
))]
```

But these traits `ArithInterp` and `CfInterp` — do they exist? Let me check what's actually exported.

Actually, re-reading the summary: "Delete convenience traits added during prior iteration (ArithInterp, CfInterp, etc.)" — these were from a prior iteration that was committed. Let me verify what's in the current code.

Looking at the test file I read (derive_macros.rs lines 17-21):
```rust
#[interpret(where(
    I::Value: kirin_arith::ArithInterp
        + kirin_cf::CfInterp
        + From<kirin_arith::ArithValue>,
))]
```

These convenience traits must exist in the committed codebase. Let me check.
<br>

Actually, looking back at the summary point 5: "must be reverted before implementing the approved design" and I did revert all uncommitted changes. The convenience traits must be from a prior committed iteration. But when I grep'd for `pub trait.*Interp`, I only found hits in `kirin-interpreter`. Let me check if these are actually compiled and referenced.

Actually wait — I need to check if the tests even compile currently. Those `ArithInterp` / `CfInterp` traits might actually exist in the lib.rs of those crates.

Let me just check quickly.

**Step 4: derive_macros.rs — after revision, remove `#[interpret(where(...))]` entirely**

With the new derive, the derive auto-generates `InnerType: Interpretable<'__ir, I>` bounds. The value-level bounds propagate through the inner types' own impls, so no manual annotation needed.

Also update the manual `CallSemantics` impl for `DerivedInterpretableDialect` (lines 57-91) to use the new signature.

**Step 5: Commit**

```
git add example/toy-lang/src/interpret.rs crates/kirin-interpreter/tests/derive_macros.rs
git commit -m "refactor: update consumers to use revised Interpretable/CallSemantics"
```

---

### Task 11: Update remaining test files

**Files:**
- Modify: `crates/kirin-interpreter/tests/error_paths.rs`
- Modify: `crates/kirin-interpreter/tests/abstract_gaps.rs`
- Modify: `crates/kirin-interpreter/tests/stage_dispatch.rs`

These files define test dialects with `#[derive(Interpretable)]` and `#[interpret(where(...))]`. Remove the `#[interpret(where(...))]` annotations. Also update any manual `Interpretable` or `CallSemantics` impls.

**Step 1: Update each file**

Remove `#[interpret(where(...))]` from all test dialect enums. Update any manual trait impls.

**Step 2: Verify tests pass**

Run: `cargo nextest run -p kirin-interpreter`

**Step 3: Commit**

```
git add crates/kirin-interpreter/tests/
git commit -m "refactor: update interpreter tests to use revised traits"
```

---

### Task 12: Full workspace build and test

**Step 1: Build**

Run: `cargo build --workspace`

**Step 2: Run all tests**

Run: `cargo nextest run --workspace`
Run: `cargo test --doc --workspace`

**Step 3: Accept any remaining snapshot changes**

Run: `cargo insta review`

**Step 4: Fix any issues**

**Step 5: Final commit if needed**

---

### Task 13: Update docs and memory

**Files:**
- Modify: `AGENTS.md` — update Interpreter Conventions section
- Modify: `.claude/projects/-Users-roger-Code-rust-kirin/memory/MEMORY.md` — update interpreter framework section

**Step 1: Update AGENTS.md**

In the "Interpreter Conventions" section, update:
- `Interpretable<'ir, I, L>` → `Interpretable<'ir, I>` with note about `L` on method
- `CallSemantics<'ir, I, L>` → `CallSemantics<'ir, I>` with note about `L` on method
- Update `'ir` lifetime pattern to note `L` is no longer on traits
- Note that `#[interpret(where(...))]` is removed
- Note that `#[derive(Interpretable)]` auto-generates `InnerType: Interpretable<'__ir, I>` bounds

**Step 2: Update MEMORY.md**

Update the Interpreter Framework section with the new trait signatures.

**Step 3: Commit**

```
git add AGENTS.md
git commit -m "docs: update AGENTS.md for Interpretable trait revision"
```
