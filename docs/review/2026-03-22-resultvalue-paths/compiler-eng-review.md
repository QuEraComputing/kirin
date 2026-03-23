# Compiler Engineer Review: Multi-Result Value Paths

**Reviewer persona:** Compiler Infrastructure Pragmatist
**Date:** 2026-03-22
**Scope:** Evaluate two design paths for supporting multiple return values in kirin's interpreter framework from an implementation cascade perspective.

---

## Current Baseline

The `Continuation<V, Ext>` enum is the central control flow type. Its largest variant today is `Fork(SmallVec<[(Block, Args<V>); 2]>)`, where `Args<V> = SmallVec<[V; 2]>`. `ResultValue` is a newtype around `Id(usize)` -- 8 bytes on 64-bit. The `Call` variant carries a single `ResultValue`.

Match sites across the crate tree: **95 occurrences of `Continuation::` across 30 files** (including docs). In source code, the critical match sites are in `exec.rs` (run_nested_calls), `transition.rs` (advance_frame_with_stage), `fixpoint.rs` (propagate_control), and the abstract interpreter's `eval_block`.

Dialect `interpret()` implementations exist in 7 crates: kirin-arith, kirin-bitwise, kirin-cf, kirin-cmp, kirin-constant, kirin-function, kirin-scf.

---

## Path A: Expand `Yield`/`Return`/`Call` to Carry Multiple Values

### Change Cascade

| File / Module | Change | LOC est. |
|---|---|---|
| `control.rs` | `Yield(V)` -> `Yield(SmallVec<[V; 1]>)`, `Return(V)` -> `Return(SmallVec<[V; 1]>)`, `Call.result` -> `Call.results: SmallVec<[ResultValue; 1]>` | ~15 |
| `stack/exec.rs` (run_nested_calls) | Return type changes from `Result<V, E>` to `Result<SmallVec<[V; 1]>, E>`. The `pending_results` stack changes to `Vec<SmallVec<[ResultValue; 1]>>`. Write-back loop writes N results instead of 1. Must handle arity mismatch (results.len() != values.len()). | ~30 |
| `stack/frame.rs` (eval_block) | `Continuation::Yield(v)` wrapping changes to `Yield(smallvec![v])`. But this is the crux: `run_nested_calls` currently returns a single `V`, and `eval_block` wraps it in `Yield`. With multi-result, does `run_nested_calls` return `SmallVec<[V; 1]>`? Then single-result callers (the common case) must always index `[0]`. | ~10 |
| `stack/transition.rs` (advance_frame_with_stage) | `Return(_)` and `Yield(_)` arms unchanged structurally (no destructuring needed), but `Call { .. }` arm must now push N results. | ~5 |
| `abstract_interp/interp.rs` (eval_block on AbstractInterpreter) | The `Call` match arm writes a single return_val via `self.write(result, ...)`. Must now write N results from N return values. | ~15 |
| `abstract_interp/fixpoint.rs` (propagate_control) | `Return(v) | Yield(v)` pattern breaks. Must join N values into N slots. `return_value: Option<V>` becomes `return_values: Option<SmallVec<[V; 1]>>` or a Vec. Pointwise join/narrow on each element. | ~35 |
| `result.rs` (AnalysisResult) | `return_value: Option<V>` -> `return_values: Option<Vec<V>>` or `SmallVec`. `is_subseteq` changes to pointwise comparison. 7 unit tests update. | ~30 |
| `call.rs` (SSACFGRegion blanket impls) | `run_nested_calls` return changes cascade here. The stack interpreter blanket returns `V` -- must change to `SmallVec<[V; 1]>` or the blanket must unpack. | ~10 |
| `ext.rs` (InterpreterExt) | `binary_op`, `unary_op`, `try_binary_op` all write a single `ResultValue`. These remain single-result. No change. | 0 |
| `value_store.rs` (ValueStore) | Add `write_results(&mut self, results: &[ResultValue], values: &[V])` or leave as-is and loop. | ~5 |
| `interpretable.rs` | Trait signature unchanged (returns `Continuation<I::Value, I::Ext>`). | 0 |
| **Dialect crates (7)** | Every `interpret()` that constructs `Continuation::Yield(v)` or `Continuation::Return(v)` must wrap in `smallvec![v]`. kirin-function's `Call` must wrap `result` in a SmallVec. kirin-scf's If/For Yield handling must unpack. | ~40 total |
| `kirin-derive-interpreter` | `Interpretable` derive generates `field_0.interpret::<__InterpL>(interpreter)` -- passes through Continuation. No change. `CallSemantics` derive generates `eval_call` forwarding -- passes through. No change. | 0 |
| `kirin-derive-toolkit` (builder_template) | Lift the `Vec<ResultValue>` and `Option<ResultValue>` rejection. Generate `SmallVec<[ResultValue; 1]>` field initialization. Each result gets `Result(stmt, idx)` as before, but packed into a SmallVec. Must change `build_result_impl` to generate struct with `SmallVec<[ResultValue; N]>` or `Vec<ResultValue>`. | ~30 |
| **Tests** | stack_interp.rs (3 match sites on Return), error_paths.rs, stage_dispatch.rs, roundtrip tests -- all must destructure SmallVec. | ~25 |

**Total estimated LOC changed: ~250**

### Derive Macro Impact

The derive macros for `Interpretable` and `CallSemantics` generate code that delegates to inner types' `interpret()` / `eval_call()`. Since the `Continuation` enum change is transparent to the method signature (it still returns `Result<Continuation<I::Value, I::Ext>, I::Error>`), **the interpreter derive macros need zero changes**. The cascading impact is entirely in hand-written dialect impls and the interpreter runtime.

The builder template in `kirin-derive-toolkit` is the real derive pain point. Today it rejects `Vec<ResultValue>` and `Option<ResultValue>` fields with compile errors. Supporting multi-result operations requires:
1. Allowing `Vec<ResultValue>` (or `SmallVec<[ResultValue; N]>`) fields.
2. Generating N SSA allocations in a loop: `for i in 0..count { stage.ssa().kind(Result(stmt_id, i))... }`.
3. The build result struct must expose a `Vec<ResultValue>` field instead of named individual results.
4. The `result_index` counter in `let_name_eq_result_value` must handle dynamic-length fields.

This is ~60-80 LOC of template changes, and the generated builder API changes shape: instead of `build_result.result_0, build_result.result_1`, callers get `build_result.results[0]`. This is a **user-facing API break** for any operation that switches from `N x ResultValue` to `Vec<ResultValue>`.

### Monomorphization Pressure

`Continuation<V, Ext>` is monomorphized for every `(V, Ext)` pair. Adding `SmallVec<[V; 1]>` to both `Yield` and `Return` changes the enum's memory layout.

Current `Yield(V)` variant payload: `size_of::<V>()`.
Proposed `Yield(SmallVec<[V; 1]>)` payload: `size_of::<V>() + size_of::<usize>() + capacity overhead`. A `SmallVec<[T; 1]>` is `max(size_of::<T>() + usize, size_of::<(usize, usize, *mut T)>())` -- typically 24 bytes on 64-bit regardless of T (due to heap-allocated fallback union). For small V (e.g., `V = i64`, 8 bytes), `Yield` goes from 8 bytes to 24 bytes.

The `Fork` variant already contains `SmallVec<[(Block, Args<V>); 2]>` which is 2 * (8 + 24) + overhead -- the Fork variant likely dominates enum size today. So in practice, the Yield/Return growth may not increase overall `size_of::<Continuation<V>>()` because the discriminant + largest variant already sets the floor. But for interpreters that never use Fork (concrete interpreters -- Fork panics), the enum was potentially smaller before. **Net impact: moderate -- the enum was already bloated by Fork, adding SmallVec to Yield/Return likely stays within the same alignment bucket.**

One real cost: `SmallVec<[V; 1]>` is not `Copy` even when `V: Copy`. This means `Yield(v)` can no longer be cheaply copied in pattern matches -- you must clone or move. Today, `Continuation::Return(v) | Continuation::Yield(v) => Some(v.clone())` works because `v` is `&V`. With SmallVec it becomes `&SmallVec<[V; 1]>` and `.clone()` allocates if len > 1.

### Compile-Time Impact

SmallVec is already in the dependency tree (used by `Args<V>` and `Fork`). No new dependencies. The trait solver work is unchanged -- no new trait bounds are introduced on `Continuation` itself. The compile-time impact is negligible.

### Error Message Quality

When a dialect author writes `Ok(Continuation::Yield(value))` (the old API), they get a type mismatch: "expected SmallVec<[V; 1]>, found V". The fix is `Ok(Continuation::Yield(smallvec![value]))`. This is a minor ergonomic tax. A `From<V> for SmallVec<[V; 1]>` impl does not exist in smallvec (and adding one would be an orphan impl). A helper constructor like `Continuation::yield_one(v)` could smooth this over, but adds API surface.

For the Call variant, `result: ResultValue` -> `results: SmallVec<[ResultValue; 1]>` is worse: the Call construction site in kirin-function currently reads cleanly:
```rust
Ok(Continuation::Call { callee, stage: stage_id, args, result: self.result() })
```
This becomes:
```rust
Ok(Continuation::Call { callee, stage: stage_id, args, results: smallvec![self.result()] })
```
Acceptable, but noisier. More importantly, if a dialect author forgets to wrap, the error points at a `SmallVec` vs `ResultValue` mismatch -- reasonable.

### Runtime Performance (Hot Path)

The `eval_block` -> `run_nested_calls` path is the interpreter hot loop. Today:
1. `run_nested_calls` returns `V`.
2. `eval_block` wraps it: `Ok(Continuation::Yield(v))`.
3. The parent (e.g., `scf.if`) matches `Continuation::Yield(v)` and calls `interp.write(self.result, v)`.

With Path A:
1. `run_nested_calls` returns `SmallVec<[V; 1]>`.
2. `eval_block` wraps it: `Ok(Continuation::Yield(values))`.
3. The parent matches `Continuation::Yield(values)`, indexes `values[0]`, and writes it.

The extra indirection through SmallVec adds a bounds check and a potential heap-allocated clone. For the common single-result case this is **one extra branch per yield** in the hot loop. The branch is perfectly predicted (always len=1 in practice until multi-result ops exist), so the CPU cost is near-zero -- but the code is uglier.

The `run_nested_calls` write-back also changes: instead of `ValueStore::write(self, result, v)`, it becomes a loop over `zip(results, values)`. For single-result, this loop executes once -- same work, more code.

### Incremental Compilation

Changes touch `kirin-interpreter` (core crate) and all 7 dialect crates. A change to `Continuation` forces recompilation of everything downstream: all dialect crates, all test crates, toy-lang, toy-qc. This is a **full rebuild** of the interpreter subgraph. One-time cost, acceptable for a semver-breaking change.

---

## Path B: Keep `Yield(V)` Single-Valued, Use Tuple/Struct Packing

### Change Cascade

| File / Module | Change | LOC est. |
|---|---|---|
| `control.rs` | No change. | 0 |
| `stack/exec.rs` | No change. | 0 |
| `stack/frame.rs` (eval_block) | No change. | 0 |
| `stack/transition.rs` | No change. | 0 |
| `abstract_interp/interp.rs` | No change. | 0 |
| `abstract_interp/fixpoint.rs` | No change. | 0 |
| `result.rs` | No change. | 0 |
| `call.rs` | No change. | 0 |
| `ext.rs` | No change. | 0 |
| `value_store.rs` | No change. | 0 |
| **kirin-scf interpret_impl.rs** | `If::interpret()` calls `eval_block`, gets `Yield(v)`, calls an unpack trait/convention to split `v` into individual values, writes each to its ResultValue. | ~20 |
| **kirin-scf lib.rs** | Add `result: ResultValue` to `If`, `results: Vec<ResultValue>` and `init_args: Vec<SSAValue>` to `For`. | ~15 |
| `kirin-derive-toolkit` (builder_template) | Lift the `Vec<ResultValue>` rejection. Generate Vec-based SSA allocation. | ~40 |
| **V type (dialect-specific)** | Need a packing convention. Options: (a) `V` is already an enum that can hold tuples (`Value::Tuple(Vec<Value>)`), or (b) add a new `Unpack` trait on V. | ~15-30 |

**Total estimated LOC changed: ~90-105**

### How It Works

The key insight: multi-result operations like `scf.if` and `scf.for` are the *consumers* of Yield, not the producers. The `Yield` terminator in the body block is produced by dialect code (e.g., `kirin-scf::Yield`) and carries one value. The parent operation's `interpret()` captures that value and decides what to do with it.

For `scf.if` with one result, the current code already works (once the result field is added):
```rust
Continuation::Yield(value) => {
    interp.write(self.result, value)?;
    Ok(Continuation::Continue)
}
```

For multi-result, the convention is that the yielded value is a "packed" representation (e.g., a tuple/struct in the value domain). The parent unpacks:
```rust
Continuation::Yield(packed) => {
    let values = packed.unpack();  // or: convention-based
    for (rv, v) in self.results.iter().zip(values) {
        interp.write(*rv, v)?;
    }
    Ok(Continuation::Continue)
}
```

This pushes the multi-value concern to the **value domain** (where it arguably belongs -- the IR already has SSAKind::Result(stmt, index) for positional indexing) rather than the control flow enum.

### Derive Macro Impact

The builder template needs the same `Vec<ResultValue>` support as Path A -- this is unavoidable because the IR struct definitions need `Vec<ResultValue>` fields regardless of how the interpreter handles them. The builder template changes are identical: ~40 LOC to generate dynamic-count SSA allocation.

The interpreter derive macros (`Interpretable`, `CallSemantics`) need **zero changes** -- same as Path A, since they only forward to inner types.

### Monomorphization Pressure

Zero change. `Continuation<V, Ext>` layout is identical to today. The `Yield(V)` variant stays the same size. No new SmallVec instantiations. No enum size growth.

### Compile-Time Impact

No change to kirin-interpreter's public types. Only kirin-scf (and other multi-result dialects) need recompilation. The builder template change in kirin-derive-toolkit triggers derive macro users to rebuild, but this is incremental -- only crates using `Vec<ResultValue>` fields rebuild.

No new trait bounds on `Continuation`. If an `Unpack` trait is introduced, it's bounded on `V` only at the dialect call site, not threaded through the Continuation or Interpreter trait hierarchy. This means **no additional trait solver work** in downstream crates.

### Error Message Quality

Dialect authors who write single-result operations see zero change. Their code continues to work identically.

Dialect authors writing multi-result operations must understand the packing convention. If `Unpack` is a trait:
```rust
I::Value: Unpack
```
Then a missing impl produces: "`MyValue` does not implement `Unpack`". Clear, actionable.

If the convention is implicit (e.g., the value domain always supports tuples), there's no type-level enforcement -- bugs are runtime panics ("tried to unpack a non-tuple value"). This is worse.

**Recommendation:** Use a trait, but make it opt-in. Only dialects that produce multi-result operations need `V: Unpack`. Add `#[diagnostic::on_unimplemented]` for a clear message.

### Runtime Performance (Hot Path)

The eval_block -> run_nested_calls -> Yield path is **completely unchanged**. Zero overhead for single-result operations. The pack/unpack cost is only paid by multi-result operations (scf.if, scf.for), and only at the yield capture site -- not on every statement.

For abstract interpretation, `propagate_control`'s `Return(v) | Yield(v)` join remains a single pointwise join on V. If V is a packed tuple, the `join` implementation handles element-wise joining internally -- no change to the fixpoint machinery.

### Incremental Compilation

Only dialect crates that add multi-result fields recompile. The interpreter core crate is untouched. This is **strictly better** than Path A for incremental compilation.

### The `Vec<ResultValue>` Builder Question

Both paths need the builder template to support `Vec<ResultValue>`. The implementation is the same either way:

1. In `let_name_eq_result_value`, detect `Collection::Vec` for `FieldCategory::Result`.
2. Generate: `let results: Vec<ResultValue> = (0..N).map(|i| stage.ssa().kind(Result(stmt_id, base_index + i)).ty(...).new().into()).collect();`
3. But N is not known at derive time -- it's runtime. So the builder function must take a `count: usize` parameter for Vec<ResultValue> fields, or the count comes from another field (like `init_args.len()`).
4. The build result struct exposes `results: Vec<ResultValue>`.

This is non-trivial but identical for both paths. ~40-60 LOC in the template, ~10 LOC in the helpers.

---

## Summary Comparison

| Dimension | Path A (SmallVec in Continuation) | Path B (Single-valued Yield + packing) |
|---|---|---|
| **LOC changed** | ~250 | ~90-105 |
| **Crates modified** | kirin-interpreter + 7 dialects + derive-toolkit | kirin-scf + derive-toolkit (+ optional trait crate) |
| **Breaking changes** | Continuation enum (semver break), all dialect match arms | Only SCF struct fields (localized break) |
| **Enum size impact** | Yield/Return grow by ~16 bytes (SmallVec overhead) | None |
| **Hot path overhead** | One extra branch + SmallVec indexing per Yield | None for single-result; pack/unpack for multi-result only |
| **Monomorphization** | SmallVec<[V; 1]> added to 2 more variants | None |
| **Compile time** | Full rebuild of interpreter subgraph | Incremental (only affected dialects) |
| **Derive macro changes** | Builder template only (~40 LOC) | Builder template only (~40 LOC) |
| **Error messages** | "expected SmallVec, found V" on old code | Clear if Unpack trait used; runtime if convention-based |
| **Correctness model** | Multi-result is first-class in control flow -- can't forget to unpack | Unpacking is dialect responsibility -- possible to forget |
| **Abstract interp impact** | AnalysisResult changes (return_value -> return_values) + pointwise join | None (join on packed V handles it internally) |
| **Future extensibility** | Any dialect can be multi-result by construction | Requires V domain to support packing |

---

## Recommendation

**Path B is the pragmatic choice.** The implementation cascade of Path A is severe: it touches every dialect, changes the hot-path enum layout, and forces a full rebuild -- all to support a feature that only 2 operations (scf.if, scf.for) currently need. Path B localizes the change to the dialects that need it, preserves the hot-path performance for the 95% of operations that are single-result, and requires ~60% less code change.

The one legitimate concern with Path B is correctness enforcement: multi-result unpacking is a dialect responsibility rather than a type-system guarantee. This is mitigated by:
1. Adding an `Unpack` trait with `#[diagnostic::on_unimplemented]` for clear errors.
2. The existing `SSAKind::Result(Statement, usize)` already models per-statement result indexing -- the IR layer already supports multi-result, only the interpreter runtime needs the convention.

If multi-result operations proliferate beyond SCF (e.g., a future `func.call` with multiple returns), Path A's first-class support would pay off. But that's a future concern -- and if it happens, the migration from B to A is straightforward because the IR/builder changes are shared.

**Start with Path B. Migrate to Path A only if multi-result becomes pervasive (>5 operations across >3 dialects).**
