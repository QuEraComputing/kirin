# Binary-Op Interpreter Helper

## Problem

Binary operation interpret impls across `kirin-arith`, `kirin-bitwise`, and `kirin-cmp` repeat an identical 5-line pattern:

```rust
let a = interp.read(*lhs)?;
let b = interp.read(*rhs)?;
interp.write(*result, a OP b)?;
Ok(Continuation::Continue)
```

This pattern appears:
- `kirin-arith`: 5 times (Add, Sub, Mul + 2 checked variants with slight differences)
- `kirin-bitwise`: 6 times (And, Or, Xor, Shl, Shr + Not which is unary)
- `kirin-cmp`: 6 times (Eq, Ne, Lt, Le, Gt, Ge)

Total: ~17 match arms with nearly identical structure. Of these, 15 are pure binary ops (read two values, apply operation, write result, continue). The 2 checked ops (Div, Rem) and 1 unary op (Neg, Not) have slight variations.

## Research Findings

### Exact shared pattern (binary, infallible)

From `kirin-arith/src/interpret_impl.rs`:
```rust
Arith::Add { lhs, rhs, result, .. } => {
    let a = interp.read(*lhs)?;
    let b = interp.read(*rhs)?;
    interp.write(*result, a + b)?;
    Ok(Continuation::Continue)
}
```

From `kirin-bitwise/src/interpret_impl.rs`:
```rust
Bitwise::And { lhs, rhs, result, .. } => {
    let a = interp.read(*lhs)?;
    let b = interp.read(*rhs)?;
    interp.write(*result, a & b)?;
    Ok(Continuation::Continue)
}
```

From `kirin-cmp/src/interpret_impl.rs`:
```rust
Cmp::Eq { lhs, rhs, result, .. } => {
    let a = interp.read(*lhs)?;
    let b = interp.read(*rhs)?;
    interp.write(*result, a.cmp_eq(&b))?;
    Ok(Continuation::Continue)
}
```

### Variations

**Checked ops** (Div, Rem in arith):
```rust
let a = interp.read(*lhs)?;
let b = interp.read(*rhs)?;
let v = a.checked_div(b).ok_or_else(|| InterpreterError::custom(DivisionByZero))?;
interp.write(*result, v)?;
Ok(Continuation::Continue)
```

**Unary ops** (Neg in arith, Not in bitwise):
```rust
let a = interp.read(*operand)?;
interp.write(*result, -a)?;
Ok(Continuation::Continue)
```

**Cmp ops**: use method calls (`a.cmp_eq(&b)`) instead of operators. The closure pattern would need `|a, b| a.cmp_eq(&b)` instead of `|a, b| a + b`.

### Generic bounds needed

A helper must be generic over:
- `I: Interpreter<'ir>` (or just `ValueStore` since we only use `read`/`write`)
- The operation: a closure `Fn(I::Value, I::Value) -> I::Value`
- The SSA values: `lhs: SSAValue`, `rhs: SSAValue`, `result: ResultValue`

The helper does NOT need `L`, `StageInfo`, or any stage-related types.

### Where to put the helper

Options:
1. **Free function in `kirin-interpreter`** -- accessible to all dialect crates
2. **Method on `Staged<'a, 'ir, I, L>`** -- only accessible within a stage scope (overkill, binary ops don't need stage)
3. **Extension trait on `ValueStore`** -- elegant, fits the pattern
4. **Free function in each dialect crate** -- no cross-crate dependency but code duplication

Recommendation: **Option 1** -- a free function (or pair of functions) in `kirin-interpreter`. Binary ops are the most common interpreter pattern, and the helper has minimal API surface.

### Lines saved estimate

Current per-arm cost: 5 lines (read, read, write, Ok).
With helper: 1 line per arm.

For 15 pure binary ops: save 60 lines (75 -> 15).
With unary helper too: save additional ~8 lines across 2 unary ops.
Checked ops don't fit the helper cleanly (different return type), so keep them manual.

**Net savings: ~65 lines across 3 crates.**

## Proposed Design

### Binary op helper

```rust
// In kirin-interpreter/src/helpers.rs (or similar)

use crate::{Continuation, ValueStore};
use kirin_ir::{ResultValue, SSAValue};

/// Execute a binary operation: read two SSA values, apply `op`, write the result.
///
/// This is the most common pattern in arithmetic, bitwise, and comparison
/// dialect interpreters.
///
/// # Example
/// ```ignore
/// Arith::Add { lhs, rhs, result, .. } => {
///     binary_op(interp, *lhs, *rhs, *result, |a, b| a + b)
/// }
/// ```
pub fn binary_op<I, F>(
    interp: &mut I,
    lhs: SSAValue,
    rhs: SSAValue,
    result: ResultValue,
    op: F,
) -> Result<Continuation<I::Value, I::Ext>, I::Error>
where
    I: ValueStore,
    F: FnOnce(I::Value, I::Value) -> I::Value,
{
    let a = interp.read(lhs)?;
    let b = interp.read(rhs)?;
    interp.write(result, op(a, b))?;
    Ok(Continuation::Continue)
}
```

### Unary op helper

```rust
/// Execute a unary operation: read one SSA value, apply `op`, write the result.
pub fn unary_op<I, F>(
    interp: &mut I,
    operand: SSAValue,
    result: ResultValue,
    op: F,
) -> Result<Continuation<I::Value, I::Ext>, I::Error>
where
    I: ValueStore,
    F: FnOnce(I::Value) -> I::Value,
{
    let a = interp.read(operand)?;
    interp.write(result, op(a))?;
    Ok(Continuation::Continue)
}
```

### Fallible binary op helper (for checked operations)

```rust
/// Execute a fallible binary operation: read two values, apply `op` which
/// may fail, write the result.
pub fn try_binary_op<I, F>(
    interp: &mut I,
    lhs: SSAValue,
    rhs: SSAValue,
    result: ResultValue,
    op: F,
) -> Result<Continuation<I::Value, I::Ext>, I::Error>
where
    I: ValueStore,
    F: FnOnce(I::Value, I::Value) -> Result<I::Value, I::Error>,
{
    let a = interp.read(lhs)?;
    let b = interp.read(rhs)?;
    let v = op(a, b)?;
    interp.write(result, v)?;
    Ok(Continuation::Continue)
}
```

### Usage after migration

**kirin-arith:**
```rust
use kirin_interpreter::helpers::{binary_op, unary_op, try_binary_op};

match self {
    Arith::Add { lhs, rhs, result, .. } => binary_op(interp, *lhs, *rhs, *result, |a, b| a + b),
    Arith::Sub { lhs, rhs, result, .. } => binary_op(interp, *lhs, *rhs, *result, |a, b| a - b),
    Arith::Mul { lhs, rhs, result, .. } => binary_op(interp, *lhs, *rhs, *result, |a, b| a * b),
    Arith::Div { lhs, rhs, result, .. } => try_binary_op(interp, *lhs, *rhs, *result, |a, b| {
        a.checked_div(b).ok_or_else(|| InterpreterError::custom(DivisionByZero).into())
    }),
    Arith::Rem { lhs, rhs, result, .. } => try_binary_op(interp, *lhs, *rhs, *result, |a, b| {
        a.checked_rem(b).ok_or_else(|| InterpreterError::custom(DivisionByZero).into())
    }),
    Arith::Neg { operand, result, .. } => unary_op(interp, *operand, *result, |a| -a),
}
```

**kirin-bitwise:**
```rust
match self {
    Bitwise::And { lhs, rhs, result, .. } => binary_op(interp, *lhs, *rhs, *result, |a, b| a & b),
    Bitwise::Or  { lhs, rhs, result, .. } => binary_op(interp, *lhs, *rhs, *result, |a, b| a | b),
    Bitwise::Xor { lhs, rhs, result, .. } => binary_op(interp, *lhs, *rhs, *result, |a, b| a ^ b),
    Bitwise::Shl { lhs, rhs, result, .. } => binary_op(interp, *lhs, *rhs, *result, |a, b| a << b),
    Bitwise::Shr { lhs, rhs, result, .. } => binary_op(interp, *lhs, *rhs, *result, |a, b| a >> b),
    Bitwise::Not { operand, result, .. } => unary_op(interp, *operand, *result, |a| !a),
}
```

**kirin-cmp:**
```rust
match self {
    Cmp::Eq { lhs, rhs, result, .. } => binary_op(interp, *lhs, *rhs, *result, |a, b| a.cmp_eq(&b)),
    Cmp::Ne { lhs, rhs, result, .. } => binary_op(interp, *lhs, *rhs, *result, |a, b| a.cmp_ne(&b)),
    // ... etc
}
```

Note: for cmp, the closure takes ownership but calls `&b`. This works because `cmp_eq` takes `&Self` and the closure can borrow from its owned value.

## Implementation Steps

1. **Create `kirin-interpreter/src/helpers.rs`** with `binary_op`, `unary_op`, and `try_binary_op` functions.
2. **Add `pub mod helpers;`** to `kirin-interpreter/src/lib.rs`.
3. **Re-export from prelude** (optional -- the helpers module path `kirin_interpreter::helpers::binary_op` is clear enough).
4. **Migrate `kirin-arith/src/interpret_impl.rs`**: replace 6 match arms.
5. **Migrate `kirin-bitwise/src/interpret_impl.rs`**: replace 6 match arms.
6. **Migrate `kirin-cmp/src/interpret_impl.rs`**: replace 6 match arms.
7. **Run tests**: `cargo nextest run -p kirin-arith -p kirin-bitwise -p kirin-cmp` and the full workspace suite.

## Risk Assessment

**Very low risk:**
- The helpers are pure convenience wrappers with no new logic.
- The function signatures are generic over `ValueStore`, which is the minimal required trait -- no unnecessary bounds.
- Existing tests cover the exact behavior being extracted.

**API stability:**
- The helpers are simple enough to be considered stable. The function signatures are unlikely to change.
- If the `Continuation` type or `ValueStore` trait changes, the helpers change with them -- this is expected and desirable.

**Performance:**
- The closure-based approach is zero-cost in release mode (monomorphized and inlined).
- No heap allocation, no dynamic dispatch.

## Testing Strategy

- **No new tests needed**: The helpers extract existing tested behavior. The existing test suites for arith, bitwise, and cmp cover all operations.
- **Verify after migration**: `cargo nextest run --workspace` must pass.
- **Optional unit tests**: A simple test in `helpers.rs` using a mock `ValueStore` could verify the helper mechanics, but this is low value given the comprehensive dialect tests.
