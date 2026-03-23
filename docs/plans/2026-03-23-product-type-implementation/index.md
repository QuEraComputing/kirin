# Product Type Implementation Plan

**Design spec**: `docs/design/multi-result-values.md`

## Goal

Replace the SmallVec-based multi-result Continuation (Waves 1-2) with
product-type-based single-valued semantics. Multi-result is syntactic sugar
over product types at the interpreter level.

## Revert vs Keep Analysis

### KEEP (from previous waves)

| Component | Commit(s) | Why |
|-----------|-----------|-----|
| Builder template `Vec<ResultValue>`/`Option<ResultValue>` | `5765dad26` | Wave 0 — needed for multi-result IR |
| `[...]` optional section syntax | `f0328a046` | Wave 0 — needed for void-if/void-return |
| SCF IR definitions (`Vec<ResultValue>`, `Vec<SSAValue>`) | `baf87f9f6` | IR faithfully represents what user wrote |
| Function IR (`Call.results`, `Return.values`) | `8c9f93d98` | Same |
| kirin-tuple crate (all structs, tests, roundtrips) | `15b4a08d4`+ | Explicit tuple operations |
| `ValueStore::read_many` / `write_many` | `7ecacbd4a`, `76fa28355` | Convenience APIs for bulk read/write |
| Roundtrip tests (SCF, Function, Tuple) | Various | Text format validation |
| Design docs and review reports | Various | Reference |

### REVERT (interpreter internals)

| File | What changes | From → To |
|------|-------------|-----------|
| `control.rs` | Return/Yield | `SmallVec<[V; 1]>` → `V` |
| `control.rs` | Call.results | **stays** `SmallVec<[ResultValue; 1]>` |
| `result.rs` | return_values | `Option<SmallVec<[V; 1]>>` → `Option<V>` |
| `call.rs` | StackInterpreter Result | `SmallVec<[V; 1]>` → `V` |
| `stack/exec.rs` | run_nested_calls return | `SmallVec<[V; 1]>` → `V` |
| `stack/call.rs` | call/call_with_stage | `SmallVec<[V; 1]>` → `V` |
| `stack/dispatch.rs` | CallDynAction::Output | `SmallVec<[V; 1]>` → `V` |
| `fixpoint.rs` | propagate_control | Pointwise SmallVec join → single value join |
| `interp.rs` | eval_block Call arm | `results`/`return_values()` → `result`/`return_value()` |
| `block_eval.rs` | doc comment | Remove SmallVec reference |

### ADD (new code)

| Component | File | Purpose |
|-----------|------|---------|
| `Product<T>` | `kirin-ir/src/product.rs` | SmallVec<[T; 2]> wrapper + iterators |
| `HasProduct` | `kirin-ir/src/product.rs` | Trait for dialect types |
| `product![]` | `kirin-ir/src/product.rs` | Construction macro |
| `ProductValue` | `kirin-interpreter/src/product_value.rs` | Trait for dialect values |
| `write_product` | `kirin-interpreter/src/product_value.rs` | Auto-destructure helper |
| `IndexValue` | `kirin-tuple/src/interpret_impl.rs` | value ↔ usize for Get/Len |

### UPDATE (dialect interpret impls)

| File | Current approach | New approach |
|------|-----------------|-------------|
| `kirin-scf/interpret_impl.rs` | `Yield(read_many(...))` | `Yield(ProductValue::new_product(values))` |
| `kirin-scf/interpret_impl.rs` | `write_many(&results, &values)` on If | `write_product(&results, v)` on If |
| `kirin-function/interpret_impl.rs` | `Return(read_many(...))` | `Return(ProductValue::new_product(values))` |
| `kirin-function/interpret_impl.rs` | `Call { results: iter().collect() }` | `Call { results: iter().collect() }` (unchanged) |
| `kirin-tuple/interpret_impl.rs` | `TupleValue` trait | Use framework `ProductValue` + add `IndexValue` |

## Wave Structure

### Wave 0: IR Foundation (Agent A, worktree)

**Crate**: `kirin-ir` only
**Time estimate**: Small — one new file, additive

Create `crates/kirin-ir/src/product.rs`:

```rust
use smallvec::SmallVec;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Product<T>(pub SmallVec<[T; 2]>);

#[macro_export]
macro_rules! product {
    ($($elem:expr),* $(,)?) => {
        $crate::Product(smallvec::smallvec![$($elem),*])
    };
}

pub trait HasProduct: Sized {
    fn from_product(product: Product<Self>) -> Self;
    fn as_product(&self) -> Option<&Product<Self>>;
}

// + iter(), iter_mut(), len(), is_empty()
// + IntoIterator (owned + borrowed), FromIterator
// + Display, Index<usize>
```

Update `crates/kirin-ir/src/lib.rs`: add `mod product; pub use product::*;`
Update `crates/kirin/src/prelude.rs`: re-export `Product`, `HasProduct`, `product`

**Validation**: `cargo check -p kirin-ir && cargo check -p kirin`

### Wave 1: Interpreter Rewrite (Agent B, worktree)

**Crate**: `kirin-interpreter` only (depends on Wave 0 merge)
**Time estimate**: Medium — ~10 file edits, 1 new file, test updates

#### Git revert strategy

`git revert 28941f49a` has conflicts in 5 files (due to later commits modifying
the same files). Agents should use **manual edits** with
`git show f0328a046:<path>` as reference for the pre-Wave-1 state. Do NOT use
`git revert`.

#### Changes

**1. `control.rs`** — Hybrid (mostly pre-Wave-1):
- `Return(V)`, `Yield(V)` — same as pre-Wave
- `Call { results: SmallVec<[ResultValue; 1]> }` — keeps multi-result slots (lightweight bookkeeping)
- This is NOT a pure restore — Call keeps `results` (plural) from Wave 1

**2. `result.rs`** — Revert to single-valued:
- `return_value: Option<V>` (not SmallVec)
- `new()` takes `Option<V>`
- `return_value()` returns `Option<&V>`
- Remove `return_values()` accessor
- `is_subseteq`: compare single return values
- Update tests: `Some(Interval::constant(42))` not `Some(smallvec![...])`

**3. `call.rs`** — Revert StackInterpreter blanket:
- `type Result = V` (not `SmallVec<[V; 1]>`)
- AbstractInterpreter blanket stays `Result = AnalysisResult<V>` (unchanged)

**4. `stack/exec.rs`** — Hybrid approach:
- `run_nested_calls` returns `V` (single value)
- `pending_results: Vec<SmallVec<[ResultValue; 1]>>` (keeps multi-result slots)
- `Continuation::Return(v) | Yield(v)` — extract single V
- Use `write_product` to destructure V into result slots
- `should_exit` returns `Ok(v)` not `Ok(values)`

**5. `stack/call.rs`** — Return type changes:
- `call -> V`, `call_with_stage -> V`

**6. `stack/dispatch.rs`** — Return type changes:
- `CallDynAction::Output = V`

**7. `abstract_interp/fixpoint.rs`** — Single-value join:
- `return_value: &mut Option<V>` (not SmallVec)
- `propagate_control`: single `join(v)` / `narrow(v)`, not pointwise

**8. `abstract_interp/interp.rs`** — eval_block Call arm:
- `Continuation::Call { results, .. }` (stays multi-slot)
- Use `return_value()` (not `return_values()`) for writing
- If single result: write directly; if multi: use `write_product`

**9. `block_eval.rs`** — Fix doc comment about eval_block return

**10. `value_store.rs`** — Change `write_many` to take `&[Self::Value]`:
```rust
fn write_many(&mut self, results: &[ResultValue], values: &[Self::Value]) -> ...
```

**11. New file `product_value.rs`**:
```rust
use kirin_ir::Product;
use crate::{InterpreterError, ValueStore};

pub trait ProductValue: Sized + Clone {
    fn as_product(&self) -> Option<&Product<Self>>;
    fn from_product(product: Product<Self>) -> Self;

    fn new_product(values: Vec<Self>) -> Self { ... }
    fn get(&self, index: usize) -> Result<Self, InterpreterError> { ... }
    fn len(&self) -> Result<usize, InterpreterError> { ... }
    fn is_empty(&self) -> Result<bool, InterpreterError> { ... }
}

pub fn write_product<V, S>(
    store: &mut S,
    results: &[kirin_ir::ResultValue],
    value: V,
) -> Result<(), S::Error>
where
    V: ProductValue,
    S: ValueStore<Value = V>,
    S::Error: From<InterpreterError>,
{ ... }
```

**12. Update `lib.rs`**: Add `mod product_value; pub use product_value::*;`

**13. Fix tests**: Update all interpreter test files:
- `stack_interp.rs`: `Return(v)` not `Return(smallvec![v])`
- `abstract_fixpoint.rs`: `AnalysisResult::new(_, _, Some(v))` not `Some(smallvec![v])`
- Others: similar mechanical fixes

**Validation**: `cargo check -p kirin-interpreter && cargo nextest run -p kirin-interpreter`

### Wave 2: Dialect Updates (3 parallel agents, worktrees — depends on Wave 1 merge)

**Agent C**: `kirin-scf` interpret_impl
- `Yield`: read SSA values via `read_many`, pack with `ProductValue::new_product`, return `Continuation::Yield(product)`. For single value, skip product wrapping. For zero values, handle void case.
- `If`: Capture `Continuation::Yield(v)`, write to results via `write_product`.
- `For`: Loop-carried state as single V (product when multiple). Unpack yielded product each iteration. Write final to results via `write_product`.
- Add `I::Value: ProductValue` bound on If and For.
- Yield does NOT need ProductValue — it just packs values and yields.

**Validation**: `cargo check -p kirin-scf`

**Agent D**: `kirin-function` interpret_impl
- `Return`: read SSA values, pack with `ProductValue::new_product`, return `Continuation::Return(product)`.
- `Call`: `results: self.results().iter().copied().collect()` — already correct (collects to SmallVec).
- Add `I::Value: ProductValue` bound on Return.

**Validation**: `cargo check -p kirin-function`

**Agent E**: `kirin-tuple` interpret_impl
- Replace `TupleValue` trait with framework's `ProductValue` from `kirin_interpreter`.
- Add `IndexValue` trait (2 methods: `as_index`, `from_index`).
- Update `NewTuple`: use `ProductValue::new_product` instead of `TupleValue::new_tuple`.
- Update `Unpack`: use `as_product()` + iterate + `write_many` instead of `TupleValue::unpack`.
- Update `Get`: use `ProductValue::get` + `IndexValue::as_index`.
- Update `Len`: use `ProductValue::len` + `IndexValue::from_index`.
- Add `kirin-interpreter` dependency to `Cargo.toml`.

**Validation**: `cargo check -p kirin-tuple`

### Wave 3: Integration (Agent F, on dev branch)

- `cargo build --workspace`
- `cargo nextest run --workspace`
- `cargo test --doc --workspace`
- Fix any integration test failures (roundtrips, toy-lang, etc.)
- Update `example/toy-lang/src/main.rs` if needed
- `cargo fmt --all`

## Dependency Graph

```
Wave 0 (A: kirin-ir)
    ↓
Wave 1 (B: kirin-interpreter) ← depends on Wave 0
    ↓
Wave 2 (C: kirin-scf, D: kirin-function, E: kirin-tuple) ← parallel, depends on Wave 1
    ↓
Wave 3 (F: integration) ← depends on Wave 2
```

## Agent Assignments

| Agent | Wave | Crate(s) | Isolation | Mode |
|-------|------|----------|-----------|------|
| A | 0 | kirin-ir | worktree | dontAsk |
| B | 1 | kirin-interpreter | worktree | dontAsk |
| C | 2 | kirin-scf | worktree | dontAsk |
| D | 2 | kirin-function | worktree | dontAsk |
| E | 2 | kirin-tuple | worktree | dontAsk |
| F | 3 | workspace | direct | dontAsk |

## Pre-Dispatch Gate Checklist

For every code-editing agent:
- [ ] `isolation: "worktree"` set (except Wave 3 which works on dev branch)
- [ ] `run_in_background: true` for Waves 0-2
- [ ] Agent prompt includes invariants block
- [ ] File assignments don't overlap with active agents

## Verification Checkpoints

After each wave merge:
1. `cargo check --workspace` — no compile errors
2. `cargo nextest run --workspace` — no test failures (may have some in waves 0-1 from downstream crates; ok if they're in crates updated in later waves)
